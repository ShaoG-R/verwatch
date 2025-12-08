use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use worker::{wasm_bindgen, Delay, Error, Fetch, Headers, Request, RequestInit, Result};

#[cfg(test)]
use std::cell::RefCell;

// =========================================================
// 常量定义
// =========================================================

const RATE_LIMIT_WAIT_SECONDS: u64 = 120;

// =========================================================
// 核心抽象层 (HTTP Interface Abstraction)
// =========================================================

#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl From<HttpMethod> for worker::Method {
    fn from(m: HttpMethod) -> Self {
        match m {
            HttpMethod::Get => worker::Method::Get,
            HttpMethod::Post => worker::Method::Post,
            HttpMethod::Put => worker::Method::Put,
            HttpMethod::Delete => worker::Method::Delete,
        }
    }
}

// 增加 Clone 以支持重试
#[derive(Clone)]
pub struct HttpRequest {
    pub url: String,
    pub method: HttpMethod,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl HttpRequest {
    pub fn new(url: &str, method: HttpMethod) -> Self {
        Self {
            url: url.to_string(),
            method,
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_body(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body.to_string());
        self
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|e| Error::from(e.to_string()))
    }
}

#[async_trait::async_trait(?Send)]
pub trait HttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse>;
}

// =========================================================
// 实现层: Worker 客户端
// =========================================================

#[derive(Clone)]
pub struct WorkerHttpClient;

#[async_trait::async_trait(?Send)]
impl HttpClient for WorkerHttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        // 使用循环处理重试逻辑
        let mut retry_count = 0;
        // 限制最大重试次数防止死循环，这里设为 1 次，即等待后重试一次
        const MAX_RETRIES: i32 = 1;

        loop {
            let headers = Headers::new();
            // 使用引用遍历，避免消耗 req.headers
            for (k, v) in &req.headers {
                headers.set(k, v)?;
            }

            let mut init = RequestInit {
                method: req.method.into(),
                headers,
                ..Default::default()
            };

            if let Some(body_str) = &req.body {
                init.body = Some(wasm_bindgen::JsValue::from_str(body_str));
            }

            let worker_req = Request::new_with_init(&req.url, &init)?;
            let mut response = Fetch::Request(worker_req).send().await?;
            let status = response.status_code();

            // 检查 403 和 Rate Limit
            if status == 403 && retry_count < MAX_RETRIES {
                let remaining = response.headers().get("X-RateLimit-Remaining")?;
                if let Some(val) = remaining {
                    // 如果剩余次数为 0，说明被限流
                    if val == "0" {
                        retry_count += 1;
                        // 等待指定时间
                        Delay::from(Duration::from_secs(RATE_LIMIT_WAIT_SECONDS)).await;
                        // 继续下一次循环进行重试
                        continue;
                    }
                }
            }

            // 正常返回（成功或非 Rate Limit 的错误）
            return Ok(HttpResponse {
                status,
                body: response.text().await?,
            });
        }
    }
}

// =========================================================
// 测试工具: MockHttpClient
// =========================================================

#[cfg(test)]
pub struct MockHttpClient {
    // (URL, (Status, Response Body))
    responses: RefCell<HashMap<String, (u16, String)>>,
    // 记录发出的请求 (URL, Method, Headers, Body)
    // 更新：添加 Headers 记录
    pub requests: RefCell<Vec<(String, String, HashMap<String, String>, Option<String>)>>,
}

#[cfg(test)]
impl MockHttpClient {
    pub fn new() -> Self {
        Self {
            responses: RefCell::new(HashMap::new()),
            requests: RefCell::new(Vec::new()),
        }
    }

    pub fn mock_response(&self, url: &str, status: u16, body: serde_json::Value) {
        self.responses
            .borrow_mut()
            .insert(url.to_string(), (status, body.to_string()));
    }
}

#[cfg(test)]
#[async_trait::async_trait(?Send)]
impl HttpClient for MockHttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        self.requests.borrow_mut().push((
            req.url.clone(),
            format!("{:?}", req.method),
            req.headers.clone(), // 记录 Headers
            req.body.clone(),
        ));

        let responses = self.responses.borrow();
        if let Some((status, body)) = responses.get(&req.url) {
            Ok(HttpResponse {
                status: *status,
                body: body.clone(),
            })
        } else {
            Ok(HttpResponse {
                status: 404,
                body: "Not Found".to_string(),
            })
        }
    }
}