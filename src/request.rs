use serde::de::DeserializeOwned;
use std::collections::HashMap;
use worker::{Error, Fetch, Headers, Request, RequestInit, Result, wasm_bindgen};

#[cfg(test)]
use reqwest;

// =========================================================
// 核心抽象层 (HTTP Interface Abstraction)
// =========================================================

/// 通用 HTTP 方法枚举
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

/// 通用 HTTP 请求结构
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

/// 通用 HTTP 响应结构
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|e| Error::from(e.to_string()))
    }
}

/// HTTP 客户端特性 (Trait)
/// 使用 async_trait 以支持异步调用，(?Send) 是因为 Worker 环境下某些类型不是 Send 的
#[async_trait::async_trait(?Send)]
pub trait HttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse>;
}

// =========================================================
// 实现层: Worker 客户端 (Production)
// =========================================================

#[derive(Clone)]
pub struct WorkerHttpClient;

#[async_trait::async_trait(?Send)]
impl HttpClient for WorkerHttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        let headers = Headers::new();
        for (k, v) in req.headers {
            headers.set(&k, &v)?;
        }

        let mut init = RequestInit {
            method: req.method.into(),
            headers,
            ..Default::default()
        };

        if let Some(body_str) = req.body {
            init.body = Some(wasm_bindgen::JsValue::from_str(&body_str));
        }

        let worker_req = Request::new_with_init(&req.url, &init)?;
        let mut response = Fetch::Request(worker_req).send().await?;

        Ok(HttpResponse {
            status: response.status_code(),
            body: response.text().await?,
        })
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

#[cfg(test)]
impl ReqwestHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait(?Send)]
impl HttpClient for ReqwestHttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        let method = match req.method {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Delete => reqwest::Method::DELETE,
        };

        let mut builder = self.client.request(method, &req.url);

        for (k, v) in req.headers {
            builder = builder.header(k, v);
        }

        if let Some(body) = req.body {
            builder = builder.body(body);
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| Error::from(format!("Reqwest Error: {}", e)))?;

        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| Error::from(format!("Reqwest Body Error: {}", e)))?;

        Ok(HttpResponse { status, body })
    }
}
