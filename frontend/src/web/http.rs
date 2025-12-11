//! HTTP 请求封装模块
//!
//! 使用 `web_sys::fetch` 替代 `gloo-net`，提供简洁的 HTTP 客户端接口。

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

/// HTTP 请求方法
#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Delete,
}

impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Delete => "DELETE",
        }
    }
}

/// HTTP 错误类型
#[derive(Debug)]
pub enum HttpError {
    /// 请求构建失败
    RequestBuildFailed(String),
    /// 网络请求失败
    NetworkError(String),
    /// 响应解析失败
    ResponseParseFailed(String),
}

impl core::fmt::Display for HttpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HttpError::RequestBuildFailed(msg) => write!(f, "请求构建失败: {}", msg),
            HttpError::NetworkError(msg) => write!(f, "网络错误: {}", msg),
            HttpError::ResponseParseFailed(msg) => write!(f, "响应解析失败: {}", msg),
        }
    }
}

/// HTTP 响应封装
pub struct HttpResponse {
    inner: Response,
}

impl HttpResponse {
    /// 获取 HTTP 状态码
    pub fn status(&self) -> u16 {
        self.inner.status()
    }

    /// 检查响应是否成功 (2xx)
    pub fn ok(&self) -> bool {
        self.inner.ok()
    }

    /// 获取响应体文本
    pub async fn text(self) -> Result<String, HttpError> {
        let promise = self
            .inner
            .text()
            .map_err(|e| HttpError::ResponseParseFailed(format!("{:?}", e)))?;

        let text = JsFuture::from(promise)
            .await
            .map_err(|e| HttpError::ResponseParseFailed(format!("{:?}", e)))?;

        text.as_string()
            .ok_or_else(|| HttpError::ResponseParseFailed("无法转换为字符串".to_string()))
    }
}

/// HTTP 请求构建器
pub struct HttpRequestBuilder {
    url: String,
    method: HttpMethod,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl HttpRequestBuilder {
    fn new(url: String, method: HttpMethod) -> Self {
        Self {
            url,
            method,
            headers: Vec::new(),
            body: None,
        }
    }

    /// 添加请求头
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    /// 设置请求体
    pub fn body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    /// 发送请求
    pub async fn send(self) -> Result<HttpResponse, HttpError> {
        let headers = Headers::new()
            .map_err(|e| HttpError::RequestBuildFailed(format!("创建 Headers 失败: {:?}", e)))?;

        for (key, value) in &self.headers {
            headers
                .set(key, value)
                .map_err(|e| HttpError::RequestBuildFailed(format!("设置 Header 失败: {:?}", e)))?;
        }

        let opts = RequestInit::new();
        opts.set_method(self.method.as_str());
        opts.set_headers(&headers.into());

        if let Some(body) = &self.body {
            opts.set_body(&JsValue::from_str(body));
        }

        let request = Request::new_with_str_and_init(&self.url, &opts)
            .map_err(|e| HttpError::RequestBuildFailed(format!("{:?}", e)))?;

        let window = web_sys::window()
            .ok_or_else(|| HttpError::NetworkError("无法获取 window 对象".to_string()))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| HttpError::NetworkError(format!("{:?}", e)))?;

        let response: Response = resp_value.dyn_into().map_err(|e| {
            HttpError::ResponseParseFailed(format!("Response 类型转换失败: {:?}", e))
        })?;

        Ok(HttpResponse { inner: response })
    }
}

/// 轻量级 HTTP 客户端
pub struct HttpClient;

impl HttpClient {
    /// 创建 GET 请求
    pub fn get(url: &str) -> HttpRequestBuilder {
        HttpRequestBuilder::new(url.to_string(), HttpMethod::Get)
    }

    /// 创建 POST 请求
    pub fn post(url: &str) -> HttpRequestBuilder {
        HttpRequestBuilder::new(url.to_string(), HttpMethod::Post)
    }

    /// 创建 DELETE 请求
    pub fn delete(url: &str) -> HttpRequestBuilder {
        HttpRequestBuilder::new(url.to_string(), HttpMethod::Delete)
    }
}
