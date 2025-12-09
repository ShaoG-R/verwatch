mod durable_object;

use verwatch_shared::ProjectConfig;
use worker::*;

// 定义仓库 Trait
#[async_trait::async_trait(?Send)]
pub trait Repository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>>;
    // 新增：批量获取配置和状态
    async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>>;

    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>>;
    async fn save_project(&self, config: &ProjectConfig) -> Result<()>;
    async fn delete_project(&self, id: &str) -> Result<()>;
    async fn toggle_pause_project(&self, id: &str) -> Result<bool>;

    // 状态管理
    async fn get_version_state(&self, key: &str) -> Result<Option<String>>;
    async fn set_version_state(&self, key: &str, value: &str) -> Result<()>;
}

// Durable Object 实现
pub struct DoProjectRepository {
    stub: Stub,
}

impl DoProjectRepository {
    pub fn new(env: &Env, binding_name: &str) -> Result<Self> {
        let namespace = env.durable_object(binding_name)?;
        // 使用单例 ID "default"
        let id = namespace.id_from_name("default")?;
        let stub = id.get_stub()?;
        Ok(Self { stub })
    }

    fn make_request_init(
        method: Method,
        body: Option<wasm_bindgen::JsValue>,
    ) -> Result<RequestInit> {
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        headers.set("Accept", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(method)
            .with_headers(headers)
            .with_body(body);

        Ok(init)
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for DoProjectRepository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
        let mut resp = self.stub.fetch_with_str("http://do/projects").await?;
        resp.json().await
    }

    async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>> {
        let mut resp = self
            .stub
            .fetch_with_str("http://do/projects/with_states")
            .await?;
        resp.json().await
    }

    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>> {
        let path = format!("http://do/projects/{}", id);
        let mut resp = self.stub.fetch_with_str(&path).await?;
        resp.json().await
    }

    async fn save_project(&self, config: &ProjectConfig) -> Result<()> {
        let body = wasm_bindgen::JsValue::from_str(&serde_json::to_string(config)?);
        let init = Self::make_request_init(Method::Post, Some(body))?;
        let req = Request::new_with_init("http://do/projects", &init)?;
        self.stub.fetch_with_request(req).await?;
        Ok(())
    }

    async fn delete_project(&self, id: &str) -> Result<()> {
        let path = format!("http://do/projects/{}", id);
        let req = Request::new(&path, Method::Delete)?;
        self.stub.fetch_with_request(req).await?;
        Ok(())
    }

    async fn toggle_pause_project(&self, id: &str) -> Result<bool> {
        // 调用 DO 端的 PATCH 接口实现原子操作
        let path = format!("http://do/projects/{}/toggle", id);
        let init = Self::make_request_init(Method::Patch, None)?;
        let req = Request::new_with_init(&path, &init)?;

        let mut resp = self.stub.fetch_with_request(req).await?;
        if resp.status_code() == 404 {
            return Err(Error::from("Project not found"));
        }
        resp.json().await
    }

    async fn get_version_state(&self, key: &str) -> Result<Option<String>> {
        let path = format!("http://do/state/{}", key);
        let mut resp = self.stub.fetch_with_str(&path).await?;
        resp.json().await
    }

    async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
        let path = format!("http://do/state/{}", key);
        let body = wasm_bindgen::JsValue::from_str(value);
        let init = Self::make_request_init(Method::Post, Some(body))?;
        let req = Request::new_with_init(&path, &init)?;
        self.stub.fetch_with_request(req).await?;
        Ok(())
    }
}

// 内存 Mock 实现（保留用于单元测试）
#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct MockRepository {
        pub data: RefCell<HashMap<String, String>>,
        pub configs: RefCell<Vec<ProjectConfig>>,
    }

    impl MockRepository {
        pub fn new() -> Self {
            Self {
                data: RefCell::new(HashMap::new()),
                configs: RefCell::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Repository for MockRepository {
        async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
            Ok(self.configs.borrow().clone())
        }

        async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>> {
            let configs = self.configs.borrow();
            let data = self.data.borrow();
            let mut result = Vec::new();

            for config in configs.iter() {
                let key = config.version_store_key();
                let state = data.get(&key).cloned();
                result.push((config.clone(), state));
            }
            Ok(result)
        }

        async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>> {
            Ok(self
                .configs
                .borrow()
                .iter()
                .find(|p| p.unique_key == id)
                .cloned())
        }

        async fn save_project(&self, config: &ProjectConfig) -> Result<()> {
            self.configs
                .borrow_mut()
                .retain(|c| c.unique_key != config.unique_key);
            self.configs.borrow_mut().push(config.clone());
            Ok(())
        }

        async fn delete_project(&self, id: &str) -> Result<()> {
            self.configs.borrow_mut().retain(|c| c.unique_key != id);
            Ok(())
        }

        async fn toggle_pause_project(&self, id: &str) -> Result<bool> {
            let mut configs = self.configs.borrow_mut();
            if let Some(c) = configs.iter_mut().find(|c| c.unique_key == id) {
                c.paused = !c.paused;
                Ok(c.paused)
            } else {
                Err(Error::from("Not Found"))
            }
        }

        async fn get_version_state(&self, key: &str) -> Result<Option<String>> {
            Ok(self.data.borrow().get(key).cloned())
        }

        async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
            self.data
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
    }
}
