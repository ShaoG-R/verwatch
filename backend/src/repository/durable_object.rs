use super::protocol::*;
use std::future::Future;
use verwatch_shared::{PREFIX_PROJECT, ProjectConfig};
use worker::*;

#[durable_object]
pub struct ProjectStore {
    state: State,
    _env: Env,
}

impl DurableObject for ProjectStore {
    fn new(state: State, _env: Env) -> Self {
        Self { state, _env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let path = req.path();

        match path.as_str() {
            ListProjectsCmd::PATH => {
                self.handle_request::<ListProjectsCmd, _, _>(req, |c| self.list_projects(c))
                    .await
            }
            ListProjectsWithStatesCmd::PATH => {
                self.handle_request::<ListProjectsWithStatesCmd, _, _>(req, |_| {
                    self.list_projects_with_states()
                })
                .await
            }
            GetProjectCmd::PATH => {
                self.handle_request::<GetProjectCmd, _, _>(req, |c| self.get_project(c))
                    .await
            }
            SaveProjectCmd::PATH => {
                self.handle_request::<SaveProjectCmd, _, _>(req, |c| self.save_project(c))
                    .await
            }
            DeleteProjectCmd::PATH => {
                self.handle_request::<DeleteProjectCmd, _, _>(req, |c| self.delete_project(c))
                    .await
            }
            TogglePauseCmd::PATH => {
                self.handle_request::<TogglePauseCmd, _, _>(req, |c| self.toggle_pause(c))
                    .await
            }
            GetVersionStateCmd::PATH => {
                self.handle_request::<GetVersionStateCmd, _, _>(req, |c| self.get_version_state(c))
                    .await
            }
            SetVersionStateCmd::PATH => {
                self.handle_request::<SetVersionStateCmd, _, _>(req, |c| self.set_version_state(c))
                    .await
            }
            _ => Response::error("Not Found", 404),
        }
    }
}

impl ProjectStore {
    // --- 类型安全辅助函数 ---
    async fn handle_request<T, F, Fut>(&self, mut req: Request, handler: F) -> Result<Response>
    where
        T: ApiRequest,
        F: FnOnce(T) -> Fut,
        Fut: Future<Output = Result<T::Response>>,
    {
        let cmd: T = match req.json().await {
            Ok(v) => v,
            Err(e) => return Response::error(format!("Invalid Request: {}", e), 400),
        };

        match handler(cmd).await {
            Ok(result) => Response::from_json(&result),
            Err(e) => Response::error(e.to_string(), 500),
        }
    }

    // --- 具体业务逻辑实现 ---

    async fn list_projects(&self, cmd: ListProjectsCmd) -> Result<Vec<ProjectConfig>> {
        let storage = self.state.storage();
        let prefix = cmd.prefix.as_deref().unwrap_or(PREFIX_PROJECT);
        let opts = ListOptions::new().prefix(prefix);
        let map = storage.list_with_options(opts).await?;

        let configs = map
            .values()
            .into_iter()
            .filter_map(|v| serde_wasm_bindgen::from_value(v.ok()?).ok())
            .collect();
        Ok(configs)
    }

    async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>> {
        let storage = &self.state.storage();
        let opts = ListOptions::new().prefix(PREFIX_PROJECT);
        let map = storage.list_with_options(opts).await?;

        let configs: Vec<ProjectConfig> = map
            .values()
            .into_iter()
            .filter_map(|v| serde_wasm_bindgen::from_value(v.ok()?).ok())
            .collect();

        // 保持原有的逻辑：引用 storage
        let tasks = configs.iter().map(|cfg| {
            let key = cfg.version_store_key();
            async move {
                let state: Option<String> = storage.get(&key).await.ok().flatten();
                state
            }
        });

        let states = futures::future::join_all(tasks).await;

        // 协议要求返回类型匹配，因此使用 into_iter() 获取所有权
        let result = configs.into_iter().zip(states.into_iter()).collect();
        Ok(result)
    }

    async fn save_project(&self, cmd: SaveProjectCmd) -> Result<()> {
        let storage = self.state.storage();
        let key = self.get_project_key(&cmd.config.unique_key);
        storage.put(&key, &cmd.config).await?;
        Ok(())
    }

    async fn get_project(&self, cmd: GetProjectCmd) -> Result<Option<ProjectConfig>> {
        let storage = self.state.storage();
        let key = self.get_project_key(&cmd.id);
        let val = storage.get(&key).await.ok().flatten();
        Ok(val)
    }

    async fn toggle_pause(&self, cmd: TogglePauseCmd) -> Result<bool> {
        let storage = self.state.storage();
        let key = self.get_project_key(&cmd.id);

        let mut config: ProjectConfig = match storage.get(&key).await {
            Ok(Some(c)) => c,
            _ => return Err(Error::from("Project not found")),
        };

        config.paused = !config.paused;
        storage.put(&key, &config).await?;
        Ok(config.paused)
    }

    async fn delete_project(&self, cmd: DeleteProjectCmd) -> Result<()> {
        let storage = self.state.storage();
        let key = self.get_project_key(&cmd.id);
        storage.delete(&key).await?;
        Ok(())
    }

    async fn get_version_state(&self, cmd: GetVersionStateCmd) -> Result<Option<String>> {
        let storage = self.state.storage();
        let val = storage.get(&cmd.key).await.ok().flatten();
        Ok(val)
    }

    async fn set_version_state(&self, cmd: SetVersionStateCmd) -> Result<()> {
        let storage = self.state.storage();
        storage.put(&cmd.key, &cmd.value).await?;
        Ok(())
    }

    fn get_project_key(&self, id: &str) -> String {
        format!("{}{}", PREFIX_PROJECT, id)
    }
}
