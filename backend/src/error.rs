use std::fmt;

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;

// =========================================================
// 错误状态枚举
// =========================================================

/// 错误状态枚举
/// 包含错误对应的语义（状态码）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchErrorStatus {
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

impl WatchErrorStatus {
    pub fn status_code(&self) -> u16 {
        match self {
            WatchErrorStatus::InvalidInput | WatchErrorStatus::Serialization => 400,
            WatchErrorStatus::Unauthorized => 401,
            WatchErrorStatus::NotFound => 404,
            WatchErrorStatus::Conflict => 409,
            WatchErrorStatus::Store => 500,
            WatchErrorStatus::ExternalApi => 502,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            WatchErrorStatus::InvalidInput => "INVALID_INPUT",
            WatchErrorStatus::Serialization => "JSON_PARSE_ERROR",
            WatchErrorStatus::Unauthorized => "UNAUTHORIZED",
            WatchErrorStatus::NotFound => "RESOURCE_NOT_FOUND",
            WatchErrorStatus::Conflict => "RESOURCE_CONFLICT",
            WatchErrorStatus::Store => "INTERNAL_STORE_ERROR",
            WatchErrorStatus::ExternalApi => "UPSTREAM_ERROR",
        }
    }
}

// =========================================================
// 错误上下文追踪
// =========================================================

/// 结构化的错误追踪片段
/// 记录错误发生时的操作和相关细节
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSpan {
    /// 操作名称，如 "storage.get", "github.fetch_release"
    pub operation: String,
    /// 额外的细节信息，如 key 名称、project id 等
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ErrorSpan {
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            detail: None,
        }
    }

    pub fn with_detail(operation: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            detail: Some(detail.into()),
        }
    }
}

// =========================================================
// 核心错误类型
// =========================================================

/// Application Domain Errors
///
/// 这是一个高内聚的错误定义，包含：
/// - status: 错误类型/语义
/// - message: 错误消息
/// - source: 原始错误（可选，用于错误链）
/// - spans: 结构化的调用追踪栈
#[derive(Debug)]
pub struct WatchError {
    pub status: WatchErrorStatus,
    pub message: String,
    /// 原始错误源（供调试用，不参与序列化）
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    /// 结构化的操作追踪
    spans: Vec<ErrorSpan>,
}

impl WatchError {
    pub fn new(status: WatchErrorStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            source: None,
            spans: Vec::new(),
        }
    }

    // --- Convenience constructors ---

    pub fn store(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::Store, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::NotFound, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::InvalidInput, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::Unauthorized, message)
    }

    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::Serialization, message)
    }

    pub fn external_api(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::ExternalApi, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(WatchErrorStatus::Conflict, message)
    }

    // --- Context builders (Builder Pattern) ---

    /// 添加操作追踪（无额外细节）
    pub fn in_op(mut self, operation: impl Into<String>) -> Self {
        self.spans.push(ErrorSpan::new(operation));
        self
    }

    /// 添加操作追踪（带额外细节）
    pub fn in_op_with(mut self, operation: impl Into<String>, detail: impl Into<String>) -> Self {
        self.spans.push(ErrorSpan::with_detail(operation, detail));
        self
    }

    /// 设置原始错误源
    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, source: E) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    // --- Accessors ---

    /// 获取对应的 HTTP 状态码
    pub fn status_code(&self) -> u16 {
        self.status.status_code()
    }

    /// 获取机器可读的错误代码
    pub fn error_code(&self) -> &'static str {
        self.status.error_code()
    }

    /// 获取错误消息
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 获取操作追踪栈
    pub fn spans(&self) -> &[ErrorSpan] {
        &self.spans
    }
}

// =========================================================
// Display & Error trait 实现
// =========================================================

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.error_code(), self.message)?;

        // 如果有 spans，追加显示
        if !self.spans.is_empty() {
            write!(f, " | trace: ")?;
            for (i, span) in self.spans.iter().enumerate() {
                if i > 0 {
                    write!(f, " -> ")?;
                }
                write!(f, "{}", span.operation)?;
                if let Some(detail) = &span.detail {
                    write!(f, "({})", detail)?;
                }
            }
        }
        Ok(())
    }
}

impl std::error::Error for WatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

pub type WatchResult<T> = std::result::Result<T, WatchError>;

// =========================================================
// 传输用错误类型
// =========================================================

/// 用于在 HTTP Header 中标识该 Response Body 是一个 ErrorResponse
pub const RPC_ERROR_HEADER: &str = "X-Rpc-Error";

/// 专用于传输的错误类型
///
/// 设计用于：
/// 1. 携带完整的错误上下文（状态、消息、追踪栈）
/// 2. 序列化为 JSON 字符串并作为 Response body 返回
/// 3. 从 Response body 中恢复并转回 WatchError
///
/// 通过直接使用 WatchErrorStatus 枚举，消除了 code 字符串映射的重复逻辑
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// 错误状态（直接序列化枚举，避免 code 字符串映射）
    pub status: WatchErrorStatus,
    /// 错误消息
    pub message: String,
    /// 结构化的操作追踪栈
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spans: Vec<ErrorSpan>,
}

impl ErrorResponse {
    pub fn new(status: WatchErrorStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            spans: Vec::new(),
        }
    }

    /// 获取 HTTP 状态码
    pub fn status_code(&self) -> u16 {
        self.status.status_code()
    }

    /// 获取机器可读的错误代码
    pub fn error_code(&self) -> &'static str {
        self.status.error_code()
    }
}

// =========================================================
// 类型转换实现
// =========================================================

impl From<WatchError> for ErrorResponse {
    fn from(e: WatchError) -> Self {
        Self {
            status: e.status,
            message: e.message,
            spans: e.spans,
        }
    }
}

impl From<ErrorResponse> for WatchError {
    fn from(e: ErrorResponse) -> Self {
        Self {
            status: e.status,
            message: e.message,
            source: None, // source 不可序列化，跨边界传输时丢失
            spans: e.spans,
        }
    }
}

impl From<worker::Error> for WatchError {
    fn from(e: worker::Error) -> Self {
        WatchError::store(e.to_string())
    }
}

impl From<verwatch_shared::serde_helper::Error> for WatchError {
    fn from(e: verwatch_shared::serde_helper::Error) -> Self {
        WatchError::serialization(e.to_string())
    }
}

impl From<JsValue> for WatchError {
    fn from(e: JsValue) -> Self {
        let msg = e.as_string().unwrap_or_else(|| format!("{:?}", e));
        WatchError::store(msg)
    }
}
