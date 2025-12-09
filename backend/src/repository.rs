mod durable_object;
pub mod protocol;
pub mod storage_adapter;

use crate::error::{AppError, Result};
use protocol::*;
use verwatch_shared::ProjectConfig;
use worker::{wasm_bindgen::JsValue, *}; // 使用自定义 Result

// 定义仓库 Trait
// 返回值修改为 crate::error::Result，解耦 worker::Error
#[async_trait::async_trait(?Send)]
pub trait Repository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>>;
    async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>>;

    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>>;
    async fn save_project(&self, config: &ProjectConfig) -> Result<()>;
    async fn delete_project(&self, id: &str) -> Result<bool>;
    async fn toggle_pause_project(&self, id: &str) -> Result<bool>;

    // 状态管理
    async fn set_version_state(&self, key: &str, value: &str) -> Result<()>;
}

// Durable Object 实现
pub struct DoProjectRepository {
    stub: Stub,
}

impl DoProjectRepository {
    pub fn new(env: &Env, binding_name: &str) -> Result<Self> {
        let namespace = env.durable_object(binding_name)?;
        let id = namespace.id_from_name("default")?;
        let stub = id.get_stub()?;
        Ok(Self { stub })
    }

    // 核心泛型方法
    async fn execute<T: ApiRequest>(&self, req: T) -> Result<T::Response> {
        // 1. 序列化请求 (map_err 处理 serde 错误)
        let body = serde_json::to_string(&req)?;

        // 2. 构造 Request
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?; // ? 自动转 AppError::Store

        let mut init = RequestInit::new();
        init.with_method(T::METHOD)
            .with_headers(headers)
            .with_body(Some(JsValue::from_str(&body)));

        let url = format!("http://do{}", T::PATH);
        let request = Request::new_with_init(&url, &init)?;

        // 3. 发送请求
        let mut response = self.stub.fetch_with_request(request).await?;

        // 4. 处理 DO 内部逻辑错误 (非 200 OK)
        if response.status_code() != 200 {
            return Err(AppError::Store(format!(
                "DO Error: {}",
                response.status_code()
            )));
        }

        // 5. 反序列化响应
        let data = response.json::<T::Response>().await?;
        Ok(data)
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for DoProjectRepository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
        self.execute(ListProjectsCmd { prefix: None }).await
    }

    async fn list_projects_with_states(&self) -> Result<Vec<(ProjectConfig, Option<String>)>> {
        self.execute(ListProjectsWithStatesCmd).await
    }

    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>> {
        self.execute(GetProjectCmd { id: id.to_string() }).await
    }

    async fn save_project(&self, config: &ProjectConfig) -> Result<()> {
        self.execute(SaveProjectCmd {
            config: config.clone(),
        })
        .await
    }

    async fn delete_project(&self, id: &str) -> Result<bool> {
        self.execute(DeleteProjectCmd { id: id.to_string() }).await
    }

    async fn toggle_pause_project(&self, id: &str) -> Result<bool> {
        self.execute(TogglePauseCmd { id: id.to_string() }).await
    }

    async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
        self.execute(SetVersionStateCmd {
            key: key.to_string(),
            value: value.to_string(),
        })
        .await
    }
}

// 内存 Mock 实现
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

        async fn delete_project(&self, id: &str) -> Result<bool> {
            if let Some(_c) = self
                .configs
                .borrow_mut()
                .iter_mut()
                .find(|c| c.unique_key == id)
            {
                self.configs.borrow_mut().retain(|c| c.unique_key != id);
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn toggle_pause_project(&self, id: &str) -> Result<bool> {
            let mut configs = self.configs.borrow_mut();
            if let Some(c) = configs.iter_mut().find(|c| c.unique_key == id) {
                c.paused = !c.paused;
                Ok(c.paused)
            } else {
                Err(AppError::NotFound("Project not found".into()))
            }
        }

        async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
            self.data
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
    }
}
