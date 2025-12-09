use crate::error::{AppError, Result};
use crate::repository::Repository;
use verwatch_shared::{CreateProjectRequest, DeleteTarget, ProjectConfig};

/// 核心业务逻辑控制器
///
/// 特点：
/// 1. 纯 Rust 实现，不依赖 worker crate (Env, Request, Response)。
/// 2. 高内聚：只关注业务规则（创建、删除、查询）。
/// 3. 易测试：可以轻松注入 MockRepository 进行单元测试。
pub struct AdminLogic<'a, R: Repository> {
    repo: &'a R,
}

impl<'a, R: Repository> AdminLogic<'a, R> {
    pub fn new(repo: &'a R) -> Self {
        Self { repo }
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
        self.repo.list_projects().await
    }

    pub async fn create_project(&self, req: CreateProjectRequest) -> Result<ProjectConfig> {
        // 业务校验示例：防止空仓库名
        if req.upstream_repo.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Upstream repo cannot be empty".into(),
            ));
        }

        let config = ProjectConfig::new(req);
        self.repo.save_project(&config).await?;
        Ok(config)
    }

    pub async fn delete_project(&self, target: DeleteTarget) -> Result<bool> {
        self.repo.delete_project(&target.id).await
    }

    pub async fn pop_project(&self, target: DeleteTarget) -> Result<Option<ProjectConfig>> {
        let current = self.repo.get_project(&target.id).await?;
        if let Some(c) = &current {
            self.repo.delete_project(&c.unique_key).await?;
        }
        Ok(current)
    }

    pub async fn toggle_pause(&self, target: DeleteTarget) -> Result<bool> {
        self.repo.toggle_pause_project(&target.id).await
    }
}

// =========================================================
// 单元测试 (无需 Miniflare/Wasm 环境)
// =========================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::MockRepository;
    use verwatch_shared::ComparisonMode;

    #[tokio::test]
    async fn test_create_project_logic() {
        let repo = MockRepository::new();
        let logic = AdminLogic::new(&repo);

        let req = CreateProjectRequest {
            upstream_owner: "rust-lang".into(),
            upstream_repo: "rust".into(),
            my_owner: "me".into(),
            my_repo: "mirror".into(),
            dispatch_token_secret: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };

        // 测试正常创建
        let result = logic.create_project(req).await.unwrap();
        assert_eq!(result.base.upstream_repo, "rust");

        // 验证副作用：确实写入了 Repo
        let stored = repo.list_projects().await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].unique_key, result.unique_key);
    }

    #[tokio::test]
    async fn test_create_project_validation() {
        let repo = MockRepository::new();
        let logic = AdminLogic::new(&repo);

        let req = CreateProjectRequest {
            upstream_owner: "rust-lang".into(),
            upstream_repo: "".into(), // 无效输入
            my_owner: "me".into(),
            my_repo: "mirror".into(),
            dispatch_token_secret: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };

        let result = logic.create_project(req).await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }
}
