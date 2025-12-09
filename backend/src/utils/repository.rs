use verwatch_shared::{PREFIX_PROJECT, ProjectConfig};
use worker::*;

#[async_trait::async_trait(?Send)]
pub trait Repository {
    // Basic CRUD
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>>;
    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>>;
    async fn save_project(&self, config: &ProjectConfig) -> Result<()>;
    async fn delete_project(&self, id: &str) -> Result<()>;

    // State Management (Versioning)
    async fn get_version_state(&self, key: &str) -> Result<Option<String>>;
    async fn set_version_state(&self, key: &str, value: &str) -> Result<()>;
}

pub struct KvProjectRepository {
    kv: KvStore,
}

impl KvProjectRepository {
    pub fn new(env: &Env, kv_binding: &str) -> Result<Self> {
        Ok(Self {
            kv: env.kv(kv_binding)?,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl Repository for KvProjectRepository {
    async fn list_projects(&self) -> Result<Vec<ProjectConfig>> {
        let list = self
            .kv
            .list()
            .prefix(PREFIX_PROJECT.to_string())
            .execute()
            .await?;

        let mut configs = Vec::new();
        let mut keys_without_meta = Vec::new();

        for key in list.keys {
            if let Some(meta) = key.metadata {
                if let Ok(cfg) = serde_json::from_value::<ProjectConfig>(meta) {
                    configs.push(cfg);
                } else {
                    keys_without_meta.push(key.name);
                }
            } else {
                keys_without_meta.push(key.name);
            }
        }

        if !keys_without_meta.is_empty() {
            let futures = keys_without_meta
                .iter()
                .map(|k| self.kv.get(k).json::<ProjectConfig>());
            let results = futures::future::join_all(futures).await;
            for res in results {
                if let Ok(Some(cfg)) = res {
                    configs.push(cfg);
                }
            }
        }

        Ok(configs)
    }

    async fn get_project(&self, id: &str) -> Result<Option<ProjectConfig>> {
        let project_key = format!("{}{}", PREFIX_PROJECT, id);
        Ok(self.kv.get(&project_key).json::<ProjectConfig>().await?)
    }

    async fn save_project(&self, config: &ProjectConfig) -> Result<()> {
        let key = format!("{}{}", PREFIX_PROJECT, config.unique_key);

        let serialized_json = serde_json::to_string(&config)?;
        let json_len = serialized_json.len();

        let mut query = self.kv.put(&key, &config)?;

        if json_len < 1024 {
            query = query.metadata(&config)?;
        } else {
            console_log!(
                "Config size ({} bytes) exceeds metadata limit (1024), skipping optimization.",
                json_len
            );
        }

        query.execute().await?;
        Ok(())
    }

    async fn delete_project(&self, id: &str) -> Result<()> {
        let project_key = format!("{}{}", PREFIX_PROJECT, id);
        self.kv.delete(&project_key).await?;
        Ok(())
    }

    async fn get_version_state(&self, key: &str) -> Result<Option<String>> {
        Ok(self.kv.get(key).text().await?)
    }

    async fn set_version_state(&self, key: &str, value: &str) -> Result<()> {
        self.kv.put(key, value)?.execute().await?;
        Ok(())
    }
}

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

    #[tokio::test]
    async fn test_basic_ops() {
        use verwatch_shared::{ComparisonMode, CreateProjectRequest};
        let repo = MockRepository::new();
        let base_config = CreateProjectRequest {
            upstream_owner: "u".into(),
            upstream_repo: "r".into(),
            my_owner: "m".into(),
            my_repo: "mr".into(),
            dispatch_token_secret: None,
            comparison_mode: ComparisonMode::PublishedAt,
        };
        let config = ProjectConfig::new(base_config);

        // Save
        repo.save_project(&config).await.unwrap();

        // Get
        let found = repo.get_project(&config.unique_key).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().unique_key, config.unique_key);

        // Delete
        repo.delete_project(&config.unique_key).await.unwrap();
        let not_found = repo.get_project(&config.unique_key).await.unwrap();
        assert!(not_found.is_none());
    }
}
