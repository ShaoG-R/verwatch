mod durable_object;
pub mod protocol;

use protocol::*;
use verwatch_shared::ProjectConfig;
use worker::{wasm_bindgen::JsValue, *};

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

    // 核心泛型方法：输入 T，必定返回 Result<T::Response>
    async fn execute<T: ApiRequest>(&self, req: T) -> Result<T::Response> {
        // 序列化请求体
        let body = serde_json::to_string(&req)?;

        // 构造 Request
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(T::METHOD)
            .with_headers(headers)
            .with_body(Some(JsValue::from_str(&body)));

        // 使用协议中定义的常量路径
        let url = format!("http://do{}", T::PATH);
        let request = Request::new_with_init(&url, &init)?;

        // 发送并处理响应
        let mut response = self.stub.fetch_with_request(request).await?;

        if response.status_code() != 200 {
            return Err(Error::from(format!("DO Error: {}", response.status_code())));
        }

        // 泛型反序列化：这里编译器保证了 json() 解析出的类型就是 T::Response
        response.json::<T::Response>().await
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for DoProjectRepository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
        // 编译器知道返回的是 Vec<ProjectConfig>
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

    async fn delete_project(&self, id: &str) -> Result<()> {
        self.execute(DeleteProjectCmd { id: id.to_string() }).await
    }

    async fn toggle_pause_project(&self, id: &str) -> Result<bool> {
        self.execute(TogglePauseCmd { id: id.to_string() }).await
    }

    async fn get_version_state(&self, key: &str) -> Result<Option<String>> {
        self.execute(GetVersionStateCmd {
            key: key.to_string(),
        })
        .await
    }

    async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
        self.execute(SetVersionStateCmd {
            key: key.to_string(),
            value: value.to_string(),
        })
        .await
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
