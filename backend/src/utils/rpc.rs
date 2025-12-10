use crate::error::{AppError, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use worker::{Headers, Method, Request, RequestInit, Response, Stub, wasm_bindgen::JsValue};

// =========================================================
// 核心 Trait 定义
// =========================================================

/// 定义请求与响应的绑定关系
/// 之前散落在 project/protocol.rs 和 repository/protocol.rs 中
pub trait ApiRequest: Serialize + DeserializeOwned {
    /// 该请求对应的响应类型
    type Response: Serialize + DeserializeOwned;
    /// DO 内部路由路径
    const PATH: &'static str;
}

// =========================================================
// RPC Client: 发送请求
// =========================================================

pub struct RpcClient {
    stub: Stub,
    // e.g. "http://monitor" or "http://registry"
    base_url: String,
}

impl RpcClient {
    pub fn new(stub: Stub, base_url: &str) -> Self {
        Self {
            stub,
            base_url: base_url.to_string(),
        }
    }

    /// 发送强类型请求并获取解析后的响应
    pub async fn send<T: ApiRequest>(&self, req: &T) -> Result<T::Response> {
        // 1. 序列化请求
        let body = serde_json::to_string(req)?;

        // 2. 构造 Headers
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;

        // 3. 构造 Request
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers);
        init.with_body(Some(JsValue::from_str(&body)));

        let url = format!("{}{}", self.base_url, T::PATH);
        let request = Request::new_with_init(&url, &init)?;

        // 4. 发送请求 (RPC 调用)
        let mut response = self.stub.fetch_with_request(request).await?;

        // 5. 检查状态码
        if response.status_code() != 200 {
            let error_text = response.text().await.unwrap_or_default();
            // 这里统一封装为 AppError::Store，保留原始 Status Code 信息
            return Err(AppError::Store(format!(
                "RPC Error [{}]: {}",
                response.status_code(),
                error_text
            )));
        }

        // 6. 反序列化响应
        // 如果响应为空，且 Response 类型为 ()，json() 可能会报错，需要处理吗？
        // 通常 worker::Response::json 会处理好，或者如果是 "Ok" 字符串等情况。
        // 原有逻辑直接调用的 json::<T::Response>()，假设协议一致。
        let data = response.json::<T::Response>().await?;
        Ok(data)
    }
}

// =========================================================
// RPC Handler: 处理请求
// =========================================================

pub struct RpcHandler;

impl RpcHandler {
    /// 统一的请求处理辅助函数
    /// 包含 Method 检查、JSON 解析、Handler 调用、错误映射
    pub async fn handle<T, F, Fut>(mut req: Request, handler: F) -> worker::Result<Response>
    where
        T: ApiRequest,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<T::Response>>,
    {
        // 1. 检查 Method
        if req.method() != Method::Post {
            return Response::error("Method Not Allowed", 405);
        }

        // 2. 健壮的 Body 解析
        // 处理空 Body 对应 Unit Struct 的情况
        let cmd: T = match req.json().await {
            Ok(v) => v,
            Err(_) => {
                if std::mem::size_of::<T>() == 0 {
                    // 对于 Unit Struct (比如 StopMonitorCmd)，允许空 Body
                    // 使用 unsafe zeroed 是 Rust 中创建 Unit Struct 的一种 hack，
                    // 但更安全的是依赖 serde_json 对 null 或 empty 的处理。
                    // 原有代码使用了该逻辑，保留以兼容。
                    unsafe { std::mem::zeroed() }
                } else {
                    return Response::error("Invalid JSON Body", 400);
                }
            }
        };

        // 3. 调用业务 Handler
        match handler(cmd).await {
            Ok(result) => Response::from_json(&result),
            Err(e) => {
                // 4. 错误处理：AppError -> HTTP Response
                let status = e.status_code();
                Response::error(e.to_string(), status)
            }
        }
    }
}
