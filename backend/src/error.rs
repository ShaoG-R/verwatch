use std::fmt;

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;

/// 错误状态枚举
/// 包含错误对应的语义（状态码）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppErrorStatus {
    /// 500: 底层基础设施错误 (如 KV/DO 读写失败, I/O 错误)
    Store,
    /// 404: 资源未找到
    NotFound,
    /// 400: 业务逻辑校验失败
    InvalidInput,
    /// 401: 鉴权失败
    Unauthorized,
    /// 400: JSON 解析或序列化错误 (专用错误类型)
    Serialization,
    /// 502: 外部 API 调用失败 (如 GitHub API 连接失败)
    ExternalApi,
    /// 409: 资源冲突 (如尝试创建已存在的 ID)
    Conflict,
}

impl AppErrorStatus {
    pub fn status_code(&self) -> u16 {
        match self {
            AppErrorStatus::InvalidInput | AppErrorStatus::Serialization => 400,
            AppErrorStatus::Unauthorized => 401,
            AppErrorStatus::NotFound => 404,
            AppErrorStatus::Conflict => 409,
            AppErrorStatus::Store => 500,
            AppErrorStatus::ExternalApi => 502,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            AppErrorStatus::InvalidInput => "INVALID_INPUT",
            AppErrorStatus::Serialization => "JSON_PARSE_ERROR",
            AppErrorStatus::Unauthorized => "UNAUTHORIZED",
            AppErrorStatus::NotFound => "RESOURCE_NOT_FOUND",
            AppErrorStatus::Conflict => "RESOURCE_CONFLICT",
            AppErrorStatus::Store => "INTERNAL_STORE_ERROR",
            AppErrorStatus::ExternalApi => "UPSTREAM_ERROR",
        }
    }
}

/// Application Domain Errors
///
/// 这是一个高内聚的错误定义，它不仅包含错误信息，还包含了错误对应的语义（状态码）。
#[derive(Debug)]
pub struct AppError {
    pub status: AppErrorStatus,
    pub message: String,
}

impl AppError {
    pub fn new(status: AppErrorStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    // Convenience constructors
    pub fn store(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::Store, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::NotFound, message)
    }
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::InvalidInput, message)
    }
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::Unauthorized, message)
    }
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::Serialization, message)
    }
    pub fn external_api(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::ExternalApi, message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(AppErrorStatus::Conflict, message)
    }

    /// 获取对应的 HTTP 状态码
    pub fn status_code(&self) -> u16 {
        self.status.status_code()
    }

    /// 获取机器可读的错误代码
    pub fn error_code(&self) -> &'static str {
        self.status.error_code()
    }

    /// 获取错误消息
    pub fn message(&self) -> String {
        self.message.clone()
    }
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

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_code(), self.message)
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;

// --- 自动转换实现 ---

impl From<worker::Error> for AppError {
    fn from(e: worker::Error) -> Self {
        AppError::store(e.to_string())
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
        // 根据 code 还原回具体的 AppError
        let status = match e.code.as_str() {
            "INVALID_INPUT" => AppErrorStatus::InvalidInput,
            "JSON_PARSE_ERROR" => AppErrorStatus::Serialization,
            "UNAUTHORIZED" => AppErrorStatus::Unauthorized,
            "RESOURCE_NOT_FOUND" => AppErrorStatus::NotFound,
            "RESOURCE_CONFLICT" => AppErrorStatus::Conflict,
            "INTERNAL_STORE_ERROR" => AppErrorStatus::Store,
            "UPSTREAM_ERROR" => AppErrorStatus::ExternalApi,
            _ => AppErrorStatus::Store,
        };

        // 如果是 fallback 的 Store 错误，但 code 本身不是 INTERNAL_STORE_ERROR，则最好把 code 保留在消息里
        let message = if matches!(status, AppErrorStatus::Store) && e.code != "INTERNAL_STORE_ERROR"
        {
            format!("[{}] {}", e.code, e.message)
        } else {
            e.message
        };

        AppError::new(status, message)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        // 专门捕获 JSON 错误，转换为 Serialization 变体
        AppError::serialization(e.to_string())
    }
}

impl From<JsValue> for AppError {
    fn from(e: JsValue) -> Self {
        // JsValue 错误（通常来自 JS 迭代器）归类为 Store 错误
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        AppError::store(msg)
    }
}
