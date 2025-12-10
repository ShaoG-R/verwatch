use std::fmt;

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;

/// Application Domain Errors
///
/// 这是一个高内聚的错误定义，它不仅包含错误信息，还包含了错误对应的语义（状态码）。
#[derive(Debug)]
pub enum AppError {
    /// 500: 底层基础设施错误 (如 KV/DO 读写失败, I/O 错误)
    Store(String),
    /// 404: 资源未找到
    NotFound(String),
    /// 400: 业务逻辑校验失败
    InvalidInput(String),
    /// 401: 鉴权失败
    Unauthorized(String),
    /// 400: JSON 解析或序列化错误 (专用错误类型)
    Serialization(String),
    /// 502: 外部 API 调用失败 (如 GitHub API 连接失败)
    ExternalApi(String),
    /// 409: 资源冲突 (如尝试创建已存在的 ID)
    Conflict(String),
}

/// 用于在 HTTP Header 中标识该 Response Body 是一个 ErrorResponse
pub const RPC_ERROR_HEADER: &str = "X-Rpc-Error";

/// 专用于传输的错误类型
///
/// 这个类型完全独立于 AppError，设计用于：
/// 1. 携带完整的错误上下文（状态码、错误码、消息）
/// 2. 序列化为 JSON 字符串并作为 Response body 返回
/// 3. 从 Response body 中恢复并转回 AppError
///
/// 这样可以绕过 worker::Error 信息不足的限制，实现跨 Worker/DO 的强类型错误传播
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
}

impl AppError {
    /// 获取对应的 HTTP 状态码
    /// 将状态码映射逻辑内聚在 Error 定义中，而不是散落在 Controller 层
    pub fn status_code(&self) -> u16 {
        match self {
            // 客户端错误
            AppError::InvalidInput(_) => 400,
            AppError::Serialization(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::NotFound(_) => 404,
            AppError::Conflict(_) => 409,

            // 服务端错误
            AppError::Store(_) => 500,
            AppError::ExternalApi(_) => 502, // Bad Gateway 通常用于上游服务错误
        }
    }

    /// 获取机器可读的错误代码 (可选，用于 API 响应结构体中)
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::InvalidInput(_) => "INVALID_INPUT",
            AppError::Serialization(_) => "JSON_PARSE_ERROR",
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::NotFound(_) => "RESOURCE_NOT_FOUND",
            AppError::Conflict(_) => "RESOURCE_CONFLICT",
            AppError::Store(_) => "INTERNAL_STORE_ERROR",
            AppError::ExternalApi(_) => "UPSTREAM_ERROR",
        }
    }

    /// 获取错误消息
    pub fn message(&self) -> String {
        match self {
            AppError::Store(msg) => msg.clone(),
            AppError::NotFound(msg) => msg.clone(),
            AppError::InvalidInput(msg) => msg.clone(),
            AppError::Unauthorized(msg) => msg.clone(),
            AppError::Serialization(msg) => msg.clone(),
            AppError::ExternalApi(msg) => msg.clone(),
            AppError::Conflict(msg) => msg.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Store(msg) => write!(f, "Store Error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            AppError::InvalidInput(msg) => write!(f, "Invalid Input: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::Serialization(msg) => write!(f, "Serialization Error: {}", msg),
            AppError::ExternalApi(msg) => write!(f, "External API Error: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;

// --- 自动转换实现 ---

impl From<worker::Error> for AppError {
    fn from(e: worker::Error) -> Self {
        AppError::Store(e.to_string())
    }
}

impl From<AppError> for ErrorResponse {
    fn from(e: AppError) -> Self {
        Self {
            status: e.status_code(),
            code: e.error_code().to_string(),
            message: e.message(),
        }
    }
}

impl From<ErrorResponse> for AppError {
    fn from(e: ErrorResponse) -> Self {
        // 根据 code 还原回具体的 AppError 变体
        // 这里的还原是“尽力而为”，因为 AppError(String) 只能携带消息
        match e.code.as_str() {
            "INVALID_INPUT" => AppError::InvalidInput(e.message),
            "JSON_PARSE_ERROR" => AppError::Serialization(e.message),
            "UNAUTHORIZED" => AppError::Unauthorized(e.message),
            "RESOURCE_NOT_FOUND" => AppError::NotFound(e.message),
            "RESOURCE_CONFLICT" => AppError::Conflict(e.message),
            "INTERNAL_STORE_ERROR" => AppError::Store(e.message),
            "UPSTREAM_ERROR" => AppError::ExternalApi(e.message),
            _ => AppError::Store(format!("[{}] {}", e.code, e.message)),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        // 专门捕获 JSON 错误，转换为 Serialization 变体
        AppError::Serialization(e.to_string())
    }
}

impl From<JsValue> for AppError {
    fn from(e: JsValue) -> Self {
        // JsValue 错误（通常来自 JS 迭代器）归类为 Store 错误
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        AppError::Store(msg)
    }
}
