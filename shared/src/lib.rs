use serde::{Deserialize, Serialize};

// =========================================================
// 常量定义 (Constants)
// =========================================================

pub const PREFIX_PROJECT: &str = "p:";
pub const PREFIX_VERSION: &str = "v:";
pub const HEADER_AUTH_KEY: &str = "X-Auth-Key";

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateProjectRequest {
    pub upstream_owner: String,
    pub upstream_repo: String,
    pub my_owner: String,
    pub my_repo: String,

    // 存储 Secret 变量名，而不是 Token 本身
    // 对应 wrangler.toml 中的 [secrets] 或 [vars]
    pub dispatch_token_secret: Option<String>,

    pub comparison_mode: ComparisonMode,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub unique_key: String,
    #[serde(flatten)]
    pub base: CreateProjectRequest,
}

impl ProjectConfig {
    pub fn new(base: CreateProjectRequest) -> Self {
        let mut config = ProjectConfig {
            unique_key: String::new(),
            base,
        };
        config.unique_key = config.generate_unique_key();
        config
    }

    pub fn version_store_key(&self) -> String {
        format!(
            "{}{}/{}",
            PREFIX_VERSION, self.base.upstream_owner, self.base.upstream_repo
        )
    }

    pub fn generate_unique_key(&self) -> String {
        format!(
            "{}/{}->{}/{}",
            self.base.upstream_owner,
            self.base.upstream_repo,
            self.base.my_owner,
            self.base.my_repo
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeleteTarget {
    pub id: String,
}
