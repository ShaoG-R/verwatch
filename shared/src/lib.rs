use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use chrono;

// =========================================================
// 常量定义 (Constants)
// =========================================================

pub mod protocol;

pub const PREFIX_VERSION: &str = "v:";
pub const HEADER_AUTH_KEY: &str = "X-Auth-Key";
pub const CHECK_INTERVAL: Duration = Duration::from_hours(1);
pub const RETRY_INTERVAL: Duration = Duration::from_secs(10);

// =========================================================
// 领域模型 (Domain Models)
// =========================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    PublishedAt,
    UpdatedAt,
}

impl Default for ComparisonMode {
    fn default() -> Self {
        ComparisonMode::PublishedAt
    }
}

/// 监控状态：暂停或运行中（附带下一次检查时间）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MonitorState {
    /// 监控已暂停
    Paused,
    /// 监控运行中，next_check_at 为下一次检查的 ISO 8601 时间 (UTC)
    Running { next_check_at: DateTime<Utc> },
}

impl Default for MonitorState {
    fn default() -> Self {
        MonitorState::Paused
    }
}

impl MonitorState {
    /// 检查是否处于暂停状态
    pub fn is_paused(&self) -> bool {
        matches!(self, MonitorState::Paused)
    }

    /// 创建一个运行中状态，使用指定的下一次检查时间
    pub fn running(next_check_at: DateTime<Utc>) -> Self {
        MonitorState::Running { next_check_at }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BaseConfig {
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,
}

impl BaseConfig {
    pub fn version_store_key(&self) -> String {
        format!(
            "{}{}/{}",
            PREFIX_VERSION, self.upstream_owner, self.upstream_repo
        )
    }

    #[inline]
    pub fn generate_unique_key(&self) -> String {
        format!(
            "{}/{}->{}/{}",
            self.upstream_owner, self.upstream_repo, self.my_owner, self.my_repo
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimeConfig {
    pub check_interval: Duration,
    pub retry_interval: Duration,
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            check_interval: CHECK_INTERVAL,
            retry_interval: RETRY_INTERVAL,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateProjectRequest {
    pub base_config: BaseConfig,

    pub time_config: TimeConfig,

    pub initial_delay: Duration,

    // 存储 Secret 变量名，而不是 Token 本身
    // 对应 wrangler.toml 中的 [secrets] 或 [vars]
    pub dispatch_token_secret: Option<String>,

    pub comparison_mode: ComparisonMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub unique_key: String,
    /// 监控状态：暂停或运行中（附带下一次检查时间）
    pub state: MonitorState,
    #[serde(flatten)]
    pub request: CreateProjectRequest,
}

impl ProjectConfig {
    pub fn new(request: CreateProjectRequest) -> Self {
        let mut config = ProjectConfig {
            unique_key: String::new(),
            state: MonitorState::Paused, // 初始状态为暂停，setup 时会更新
            request,
        };
        config.unique_key = config.generate_unique_key();
        config
    }

    #[inline]
    pub fn version_store_key(&self) -> String {
        self.request.base_config.version_store_key()
    }

    #[inline]
    pub fn generate_unique_key(&self) -> String {
        self.request.base_config.generate_unique_key()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeleteTarget {
    pub id: String,
}
