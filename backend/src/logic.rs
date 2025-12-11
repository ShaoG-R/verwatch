use crate::error::{WatchError, WatchResult};
use crate::repository::Registry;
use verwatch_shared::{CreateProjectRequest, DeleteTarget, ProjectConfig};

/// 管理端业务逻辑控制器
///
/// 特点：
/// 1. 纯 Rust 实现，不依赖 worker crate (Env, Request, Response)。
/// 2. 高内聚：只关注业务规则（创建、删除、查询）。
/// 3. 易测试：可以轻松注入 MockRegistry 进行单元测试。
pub struct AdminLogic<'a, R: Registry> {
    registry: &'a R,
}

impl<'a, R: Registry> AdminLogic<'a, R> {
    pub fn new(registry: &'a R) -> Self {
        Self { registry }
    }

    /// 列出所有项目
    pub async fn list_projects(&self) -> WatchResult<Vec<ProjectConfig>> {
        self.registry
            .list()
            .await
            .map_err(|e| e.in_op("admin.list"))
    }

    /// 创建项目
    /// 1. 校验输入
    /// 2. 构建 ProjectConfig
    /// 3. 通过 Registry 注册 (Registry 内部会调用 Monitor.setup)
    pub async fn create_project(&self, req: CreateProjectRequest) -> WatchResult<ProjectConfig> {
        // 业务校验：防止空仓库名
        if req.base_config.upstream_repo.trim().is_empty() {
            return Err(WatchError::invalid_input("Upstream repo cannot be empty")
                .in_op("admin.create.validate"));
        }

        let config = ProjectConfig::new(req);
        let unique_key = config.unique_key.clone();

        // 检查是否已存在
        if self
            .registry
            .is_registered(&unique_key)
            .await
            .map_err(|e| e.in_op_with("admin.create.check", &unique_key))?
        {
            return Err(
                WatchError::conflict(format!("Project '{}' already exists", unique_key))
                    .in_op("admin.create"),
            );
        }

        // 注册 (内部调用 Monitor.setup)
        self.registry
            .register(&config)
            .await
            .map_err(|e| e.in_op_with("admin.create.register", &unique_key))?;

        Ok(config)
    }

    /// 删除项目
    /// 通过 Registry 注销 (Registry 内部会调用 Monitor.stop)
    pub async fn delete_project(&self, target: DeleteTarget) -> WatchResult<bool> {
        self.registry
            .unregister(&target.id)
            .await
            .map_err(|e| e.in_op_with("admin.delete", &target.id))
    }

    /// 弹出项目 (获取并删除)
    pub async fn pop_project(&self, target: DeleteTarget) -> WatchResult<Option<ProjectConfig>> {
        // 先获取
        let projects = self
            .registry
            .list()
            .await
            .map_err(|e| e.in_op_with("admin.pop.list", &target.id))?;
        let config = projects.into_iter().find(|c| c.unique_key == target.id);

        if let Some(ref c) = config {
            self.registry
                .unregister(&c.unique_key)
                .await
                .map_err(|e| e.in_op_with("admin.pop.unregister", &c.unique_key))?;
        }

        Ok(config)
    }

    /// 切换监控状态
    pub async fn switch_monitor(&self, unique_key: String, paused: bool) -> WatchResult<bool> {
        self.registry
            .switch_monitor(&unique_key, paused)
            .await
            .map_err(|e| e.in_op_with("admin.switch", &unique_key))
    }

    /// 手动触发检查
    pub async fn trigger_check(&self, unique_key: String) -> WatchResult<bool> {
        self.registry
            .trigger_check(&unique_key)
            .await
            .map_err(|e| e.in_op_with("admin.trigger", &unique_key))
    }
}

// =========================================================
// 单元测试 (无需 Miniflare/Wasm 环境)
// =========================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{error::WatchErrorStatus, repository::tests::MockRegistry};
    use verwatch_shared::{BaseConfig, ComparisonMode, TimeConfig};

    fn make_request(upstream_repo: &str) -> CreateProjectRequest {
        CreateProjectRequest {
            base_config: BaseConfig {
                upstream_owner: "rust-lang".into(),
                upstream_repo: upstream_repo.into(),
                my_owner: "me".into(),
                my_repo: "mirror".into(),
            },
            time_config: TimeConfig::default(),
            comparison_mode: ComparisonMode::PublishedAt,
            dispatch_token_secret: None,
            initial_delay: std::time::Duration::from_secs(60),
        }
    }

    #[tokio::test]
    async fn test_create_project_logic() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 测试正常创建
        let result = logic.create_project(make_request("rust")).await.unwrap();
        assert_eq!(result.request.base_config.upstream_repo, "rust");

        // 验证副作用：确实注册了
        let stored = registry.list().await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].unique_key, result.unique_key);
    }

    #[tokio::test]
    async fn test_create_project_validation() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 无效输入
        let result = logic.create_project(make_request("")).await;
        assert!(matches!(
            result,
            Err(WatchError {
                status: WatchErrorStatus::InvalidInput,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_create_project_conflict() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 第一次创建成功
        logic.create_project(make_request("rust")).await.unwrap();

        // 第二次创建应冲突
        let result = logic.create_project(make_request("rust")).await;
        assert!(matches!(
            result,
            Err(WatchError {
                status: WatchErrorStatus::Conflict,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn test_delete_project() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 创建项目
        let config = logic.create_project(make_request("rust")).await.unwrap();

        // 删除项目
        let deleted = logic
            .delete_project(DeleteTarget {
                id: config.unique_key.clone(),
            })
            .await
            .unwrap();
        assert!(deleted);

        // 验证已删除
        let list = registry.list().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_pop_project() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 创建项目
        let config = logic.create_project(make_request("rust")).await.unwrap();

        // Pop 项目
        let popped = logic
            .pop_project(DeleteTarget {
                id: config.unique_key.clone(),
            })
            .await
            .unwrap();

        assert!(popped.is_some());
        assert_eq!(popped.unwrap().unique_key, config.unique_key);

        // 验证已删除
        let list = registry.list().await.unwrap();

        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_switch_monitor() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 创建项目 (默认暂停)
        let config = logic.create_project(make_request("rust")).await.unwrap();
        assert!(config.state.is_paused());

        // 切换为运行
        let switched = logic
            .switch_monitor(config.unique_key.clone(), false)
            .await
            .unwrap();
        assert!(switched);

        // 验证状态
        let list = registry.list().await.unwrap();
        assert!(!list[0].state.is_paused());
    }

    #[tokio::test]
    async fn test_trigger_check() {
        let registry = MockRegistry::new();
        let logic = AdminLogic::new(&registry);

        // 创建项目
        let config = logic.create_project(make_request("rust")).await.unwrap();

        // 触发检查
        let triggered = logic
            .trigger_check(config.unique_key.clone())
            .await
            .unwrap();
        assert!(triggered);
    }
}
