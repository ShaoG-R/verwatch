use super::storage_adapter::{MapTrait, StorageAdapter, WorkerStorage};
use crate::repository::protocol::*;
use std::future::Future;
use verwatch_shared::{PREFIX_PROJECT, ProjectConfig};
use worker::*;

// =========================================================
// 核心业务逻辑 (可测试)
// =========================================================

// 泛型 S 允许注入 MockStorage 或 WorkerStorage
pub struct ProjectStoreLogic<S: StorageAdapter> {
    storage: S,
}

impl<S: StorageAdapter> ProjectStoreLogic<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    fn get_project_key(&self, id: &str) -> String {
        format!("{}{}", PREFIX_PROJECT, id)
    }

    pub async fn list_projects(&self, cmd: ListProjectsCmd) -> Result<Vec<ProjectConfig>> {
        let prefix = cmd.prefix.as_deref().unwrap_or(PREFIX_PROJECT);
        let map = self.storage.list_map::<ProjectConfig>(prefix).await?;
        let configs: Vec<ProjectConfig> = map.into_values().collect::<Result<_>>()?;

        Ok(configs)
    }

    pub async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>> {
        let map = self
            .storage
            .list_map::<ProjectConfig>(PREFIX_PROJECT)
            .await?;
        let configs: Vec<ProjectConfig> = map.into_values().collect::<Result<_>>()?;

        // 2. 聚合获取所有 State (并发 N+1)
        // 这里的闭包捕获 &self.storage。
        // 对于 WorkerStorage，底层是 Promise，天然并发。
        // 对于 MockStorage，RefCell 允许多个不可变借用(borrow)，因此也是安全的。
        let tasks = configs.iter().map(|cfg| {
            let key = cfg.version_store_key();
            async move {
                // 保持原有逻辑：忽略错误（如key不存在），视为 None
                self.storage.get::<String>(&key).await.unwrap_or(None)
            }
        });

        // 核心优化：并发等待所有结果
        let states = futures::future::join_all(tasks).await;

        let result = configs.into_iter().zip(states.into_iter()).collect();
        Ok(result)
    }

    pub async fn get_project(&self, cmd: GetProjectCmd) -> Result<Option<ProjectConfig>> {
        let key = self.get_project_key(&cmd.id);
        self.storage.get(&key).await
    }

    pub async fn save_project(&self, cmd: SaveProjectCmd) -> Result<()> {
        let key = self.get_project_key(&cmd.config.unique_key);
        self.storage.put(&key, &cmd.config).await
    }

    pub async fn delete_project(&self, cmd: DeleteProjectCmd) -> Result<bool> {
        let key = self.get_project_key(&cmd.id);
        self.storage.delete(&key).await
    }

    pub async fn toggle_pause(&self, cmd: TogglePauseCmd) -> Result<bool> {
        let key = self.get_project_key(&cmd.id);

        let mut config: ProjectConfig = match self.storage.get(&key).await? {
            Some(c) => c,
            None => return Err(Error::from("Project not found")),
        };

        config.paused = !config.paused;
        self.storage.put(&key, &config).await?;
        Ok(config.paused)
    }

    pub async fn get_version_state(&self, cmd: GetVersionStateCmd) -> Result<Option<String>> {
        self.storage.get(&cmd.key).await
    }

    pub async fn set_version_state(&self, cmd: SetVersionStateCmd) -> Result<()> {
        self.storage.put(&cmd.key, &cmd.value).await
    }
}

// =========================================================
// Durable Object 壳 (平台相关)
// =========================================================

#[durable_object]
pub struct ProjectStore {
    // 这里持有 Logic 实例，绑定具体的 WorkerStorage
    logic: ProjectStoreLogic<WorkerStorage>,
    _env: Env,
}

impl DurableObject for ProjectStore {
    fn new(state: State, env: Env) -> Self {
        let storage = WorkerStorage(state.storage());
        Self {
            logic: ProjectStoreLogic::new(storage),
            _env: env,
        }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();

        // 路由分发直接调用 self.logic 的方法
        match path.as_str() {
            ListProjectsCmd::PATH => self.handle_req(req, |c| self.logic.list_projects(c)).await,
            ListProjectsWithStatesCmd::PATH => {
                self.handle_req(req, |_: ListProjectsWithStatesCmd| {
                    self.logic.list_projects_with_states()
                })
                .await
            }
            GetProjectCmd::PATH => self.handle_req(req, |c| self.logic.get_project(c)).await,
            SaveProjectCmd::PATH => self.handle_req(req, |c| self.logic.save_project(c)).await,
            DeleteProjectCmd::PATH => self.handle_req(req, |c| self.logic.delete_project(c)).await,
            TogglePauseCmd::PATH => self.handle_req(req, |c| self.logic.toggle_pause(c)).await,
            GetVersionStateCmd::PATH => {
                self.handle_req(req, |c| self.logic.get_version_state(c))
                    .await
            }
            SetVersionStateCmd::PATH => {
                self.handle_req(req, |c| self.logic.set_version_state(c))
                    .await
            }
            _ => Response::error("Not Found", 404),
        }
    }
}

impl ProjectStore {
    // 泛型辅助函数：负责 JSON 解析和错误处理
    async fn handle_req<T, F, Fut>(&self, mut req: Request, handler: F) -> Result<Response>
    where
        T: ApiRequest,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<T::Response>>,
    {
        // 某些请求可能没有 Body (如 ListProjectsWithStatesCmd)，
        // 这里的处理逻辑需要根据 cmd 类型稍作适配，或者前端统一发 {}
        let cmd: T = match req.json().await {
            Ok(v) => v,
            Err(_) => {
                // 如果 body 为空但 T 是 Unit Struct (如 ListProjectsWithStatesCmd)，尝试默认构造
                // 这里简单起见，假设前端总是发送合法的 JSON
                return Response::error("Invalid JSON Request", 400);
            }
        };

        match handler(cmd).await {
            Ok(result) => Response::from_json(&result),
            Err(e) => Response::error(e.to_string(), 500),
        }
    }
}

// =========================================================
// 单元测试 (Pure Rust)
// =========================================================
#[cfg(test)]
mod tests {
    use super::super::storage_adapter::MockStorage;
    use super::*;

    #[tokio::test]
    async fn test_logic_flow() {
        // 1. 设置 Mock
        let storage = MockStorage::new();
        let logic = ProjectStoreLogic::new(storage);

        // 2. 准备数据
        let config = ProjectConfig {
            unique_key: "test-id".into(),
            base: verwatch_shared::CreateProjectRequest {
                upstream_owner: "u".into(),
                upstream_repo: "r".into(),
                my_owner: "m".into(),
                my_repo: "mr".into(),
                dispatch_token_secret: None,
                comparison_mode: verwatch_shared::ComparisonMode::PublishedAt,
            },
            paused: false,
        };

        // 3. 测试 Save
        logic
            .save_project(SaveProjectCmd {
                config: config.clone(),
            })
            .await
            .unwrap();

        // 4. 测试 List
        let list = logic
            .list_projects(ListProjectsCmd { prefix: None })
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].unique_key, "test-id");

        // 5. 测试 Toggle Pause
        let new_state = logic
            .toggle_pause(TogglePauseCmd {
                id: "test-id".into(),
            })
            .await
            .unwrap();
        assert_eq!(new_state, true);

        let updated = logic
            .get_project(GetProjectCmd {
                id: "test-id".into(),
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.paused, true);
    }
}
