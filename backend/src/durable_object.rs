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

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let path = req.path();
        let method = req.method();
        let storage = self.state.storage();

        match (method, path.as_str()) {
            // List all projects (优化：使用底层前缀过滤)
            (Method::Get, "/projects") => {
                let opts = ListOptions::new().prefix(PREFIX_PROJECT);
                let map = storage.list_with_options(opts).await?;

                let configs: Vec<ProjectConfig> = map
                    .values()
                    .into_iter()
                    .filter_map(|v| serde_wasm_bindgen::from_value(v.ok()?).ok())
                    .collect();

                Response::from_json(&configs)
            }

            // =========================================================
            // Optimization: 批量获取配置+状态 (Bulk Get)
            // =========================================================
            (Method::Get, "/projects/with_states") => {
                // 1. 获取所有项目配置
                let opts = ListOptions::new().prefix(PREFIX_PROJECT);
                let map = storage.list_with_options(opts).await?;

                let configs: Vec<ProjectConfig> = map
                    .values()
                    .into_iter()
                    .filter_map(|v| serde_wasm_bindgen::from_value(v.ok()?).ok())
                    .collect();

                // 2. 并发读取所有项目对应的版本状态
                // 此时是在 DO 内部进行 IO，延迟极低
                let tasks = configs.iter().map(|cfg| {
                    let key = cfg.version_store_key();
                    let storage = &storage;
                    async move {
                        let state: Option<String> = storage.get(&key).await.ok().flatten();
                        state
                    }
                });

                let states = futures::future::join_all(tasks).await;

                // 3. 组装结果: Vec<(ProjectConfig, Option<String>)>
                let result: Vec<(&ProjectConfig, Option<String>)> =
                    configs.iter().zip(states.into_iter()).collect();

                Response::from_json(&result)
            }

            // Get single project
            (Method::Get, path) if path.starts_with("/projects/") => {
                let id = &path["/projects/".len()..];
                let key = self.get_project_key(id);
                let val: Option<ProjectConfig> = storage.get(&key).await.ok().flatten();
                Response::from_json(&val)
            }

            // Save project
            (Method::Post, "/projects") => {
                let config: ProjectConfig = req.json().await?;
                let key = self.get_project_key(&config.unique_key);
                storage.put(&key, &config).await?;
                Response::ok("Saved")
            }

            // Atomic Toggle Pause (优化：原子操作减少 RTT)
            (Method::Patch, path) if path.contains("/toggle") => {
                let id = path
                    .trim_start_matches("/projects/")
                    .trim_end_matches("/toggle");
                let key = self.get_project_key(id);

                let mut config: ProjectConfig = match storage.get(&key).await {
                    Ok(Some(c)) => c,
                    Ok(None) => return Response::error("Project not found", 404),
                    Err(_) => return Response::error("Project not found", 404),
                };

                config.paused = !config.paused;
                storage.put(&key, &config).await?;
                Response::from_json(&config.paused)
            }

            // Delete project
            (Method::Delete, path) if path.starts_with("/projects/") => {
                let id = &path["/projects/".len()..];
                let key = self.get_project_key(id);
                storage.delete(&key).await?;
                Response::ok("Deleted")
            }

            // State/Version management
            (Method::Get, path) if path.starts_with("/state/") => {
                let key = &path["/state/".len()..];
                // Repository 接口期望 Option<String>
                let val: Option<String> = storage.get(key).await.ok().flatten();
                Response::from_json(&val)
            }

            (Method::Post, path) if path.starts_with("/state/") => {
                let key = &path["/state/".len()..];
                let val: String = req.text().await?;
                storage.put(key, val).await?;
                Response::ok("State Saved")
            }

            _ => Response::error("Not Found", 404),
        }
    }
}

impl ProjectStore {
    fn get_project_key(&self, id: &str) -> String {
        format!("{}{}", PREFIX_PROJECT, id)
    }
}
