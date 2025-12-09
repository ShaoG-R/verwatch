use std::fmt;

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
        // worker::Error 比较宽泛，统一归类为 Store/Infrastructure 错误
        AppError::Store(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        // 专门捕获 JSON 错误，转换为 Serialization 变体
        AppError::Serialization(e.to_string())
    }
}
