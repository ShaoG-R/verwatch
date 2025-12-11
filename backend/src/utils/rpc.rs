use crate::error::{WatchError, WatchResult};

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
    pub async fn send<T: ApiRequest>(&self, req: &T) -> WatchResult<T::Response> {
        // 1. 序列化请求
        let body = serde_json_wasm::to_string(req).map_err(|e| {
            WatchError::serialization(e.to_string()).in_op_with("rpc.serialize", T::PATH)
        })?;

        // 2. 构造 Headers
        let headers = Headers::new();
        headers
            .set("Content-Type", "application/json")
            .map_err(|e| WatchError::from(e).in_op("rpc.headers"))?;

        // 3. 构造 Request
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers);
        init.with_body(Some(JsValue::from_str(&body)));

        let url = format!("{}{}", self.base_url, T::PATH);
        let request = Request::new_with_init(&url, &init)
            .map_err(|e| WatchError::from(e).in_op_with("rpc.request", T::PATH))?;

        // 4. 发送请求 (RPC 调用)
        let mut response = self
            .stub
            .fetch_with_request(request)
            .await
            .map_err(|e| WatchError::from(e).in_op_with("rpc.fetch", T::PATH))?;

        // 5. 检查状态码
        if response.status_code() != 200 {
            let error_text = response.text().await.unwrap_or_default();

            // 检查特定的 Header，以确定这是一个我们自己生成的结构化错误响应
            // 这可以防止将普通的 HTTP 错误（如 Cloudflare 报错页面）误判为 JSON
            let is_rpc_error = response
                .headers()
                .get(crate::error::RPC_ERROR_HEADER)
                .ok()
                .flatten()
                .is_some();

            if is_rpc_error {
                // 尝试恢复为强类型 WatchError (已携带远端上下文)
                if let Ok(error_response) =
                    serde_json_wasm::from_str::<crate::error::ErrorResponse>(&error_text)
                {
                    // 将远端错误转回 WatchError，并追加本地 RPC 调用上下文
                    return Err(WatchError::from(error_response).in_op_with("rpc.call", T::PATH));
                }
            }

            // Fallback: 统一封装为 WatchError::Store
            return Err(WatchError::store(format!(
                "RPC Error [{}]: {}",
                response.status_code(),
                error_text
            ))
            .in_op_with("rpc.call", T::PATH));
        }

        // 6. 反序列化响应
        let data = response
            .json::<T::Response>()
            .await
            .map_err(|e| WatchError::from(e).in_op_with("rpc.deserialize", T::PATH))?;
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
        Fut: Future<Output = WatchResult<T::Response>>,
    {
        // 1. 检查 Method
        if req.method() != Method::Post {
            return Response::error("Method Not Allowed", 405);
        }

        // 2. 健壮的 Body 解析
        let text = match req.text().await {
            Ok(t) => t,
            Err(e) => return Response::error(format!("Failed to read body: {}", e), 400),
        };

        let cmd_result = if text.trim().is_empty() {
            serde_json_wasm::from_str("null")
        } else {
            serde_json_wasm::from_str(&text)
        };

        let cmd: T = match cmd_result {
            Ok(v) => v,
            Err(e) => return Response::error(format!("Invalid JSON Body: {}", e), 400),
        };

        // 3. 调用业务 Handler
        match handler(cmd).await {
            Ok(result) => Response::from_json(&result),
            Err(e) => {
                // 4. 错误处理：将错误转换为 ErrorResponse 并作为 JSON 响应返回
                // 这样客户端可以通过 Deserialize 还原回原始的 WatchError (包含 Status Code 等)
                use crate::error::{ErrorResponse, RPC_ERROR_HEADER};
                let error_response: ErrorResponse = e.into();
                let status = error_response.status_code();

                match Response::from_json(&error_response) {
                    Ok(mut resp) => {
                        // 设置 Header 标识这是一个结构化错误响应
                        // 客户端收到这个 Header 才会尝试解析 JSON ErrorResponse
                        let _ = resp.headers_mut().set(RPC_ERROR_HEADER, "true");
                        Ok(resp.with_status(status))
                    }
                    Err(serde_err) => {
                        Response::error(format!("Failed to serialize error: {}", serde_err), 500)
                    }
                }
            }
        }
    }
}
