use std::time::Duration;

use crate::error::WatchResult;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};

/// 抽象存储接口：负责数据的持久化
#[async_trait(?Send)]
pub trait StorageAdapter {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> WatchResult<Option<T>>;
    async fn put<T: Serialize>(&self, key: &str, value: &T) -> WatchResult<()>;
    async fn delete(&self, key: &str) -> WatchResult<bool>;
}

/// 抽象环境变量接口：负责访问环境变量和 secrets
pub trait EnvAdapter {
    /// 获取环境变量
    fn var(&self, name: &str) -> Option<String>;
    /// 获取 secret
    fn secret(&self, name: &str) -> Option<String>;
}

/// 抽象调度接口：负责定时任务 (Alarm)
#[async_trait(?Send)]
pub trait AlarmScheduler {
    /// 设置下一次唤醒的时间戳 (毫秒)
    async fn set_alarm(&self, scheduled_time: Duration) -> WatchResult<()>;
    /// 删除当前的闹钟
    async fn delete_alarm(&self) -> WatchResult<()>;
}

pub struct WorkerStorage(pub worker::Storage);

#[async_trait(?Send)]
impl StorageAdapter for WorkerStorage {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> WatchResult<Option<T>> {
        self.0.get(key).await.or_else(|e| {
            let msg = e.to_string();
            if msg.contains("No such value") {
                Ok(None)
            } else {
                Err(crate::error::WatchError::from(e).in_op_with("storage.get", key))
            }
        })
    }

    async fn put<T: Serialize>(&self, key: &str, value: &T) -> WatchResult<()> {
        self.0
            .put(key, value)
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("storage.put", key))
    }

    async fn delete(&self, key: &str) -> WatchResult<bool> {
        self.0
            .delete(key)
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op_with("storage.delete", key))
    }
}

#[async_trait(?Send)]
impl AlarmScheduler for WorkerStorage {
    async fn set_alarm(&self, scheduled_time: Duration) -> WatchResult<()> {
        self.0
            .set_alarm(scheduled_time)
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op("alarm.set"))
    }

    async fn delete_alarm(&self) -> WatchResult<()> {
        self.0
            .delete_alarm()
            .await
            .map_err(|e| crate::error::WatchError::from(e).in_op("alarm.delete"))
    }
}

/// Worker Env 的 EnvAdapter 实现
pub struct WorkerEnv<'a>(pub &'a worker::Env);

impl<'a> EnvAdapter for WorkerEnv<'a> {
    fn var(&self, name: &str) -> Option<String> {
        self.0.var(name).ok().map(|v| v.to_string())
    }

    fn secret(&self, name: &str) -> Option<String> {
        self.0.secret(name).ok().map(|s| s.to_string())
    }
}

// =========================================================
// 测试环境实现 (Mock)
// =========================================================

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct MockStorage {
        pub map: RefCell<HashMap<String, String>>,
        pub alarm: RefCell<Option<Duration>>,
    }

    impl MockStorage {
        pub fn new() -> Self {
            Self {
                map: RefCell::new(HashMap::new()),
                alarm: RefCell::new(None),
            }
        }
    }

    /// Mock 环境变量适配器
    pub struct MockEnv {
        vars: HashMap<String, String>,
        secrets: HashMap<String, String>,
    }

    impl MockEnv {
        pub fn new() -> Self {
            Self {
                vars: HashMap::new(),
                secrets: HashMap::new(),
            }
        }

        pub fn with_var(mut self, name: &str, value: &str) -> Self {
            self.vars.insert(name.to_string(), value.to_string());
            self
        }

        pub fn with_secret(mut self, name: &str, value: &str) -> Self {
            self.secrets.insert(name.to_string(), value.to_string());
            self
        }
    }

    impl EnvAdapter for MockEnv {
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).cloned()
        }

        fn secret(&self, name: &str) -> Option<String> {
            self.secrets.get(name).cloned()
        }
    }

    #[async_trait(?Send)]
    impl StorageAdapter for MockStorage {
        async fn get<T: DeserializeOwned>(&self, key: &str) -> WatchResult<Option<T>> {
            let map = self.map.borrow();
            if let Some(val_str) = map.get(key) {
                let val = serde_json::from_str(val_str)?;
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }

        async fn put<T: Serialize>(&self, key: &str, value: &T) -> WatchResult<()> {
            let val_str = serde_json::to_string(value)?;
            self.map.borrow_mut().insert(key.to_string(), val_str);
            Ok(())
        }

        async fn delete(&self, key: &str) -> WatchResult<bool> {
            Ok(self.map.borrow_mut().remove(key).is_some())
        }
    }

    #[async_trait(?Send)]
    impl AlarmScheduler for MockStorage {
        async fn set_alarm(&self, scheduled_time: Duration) -> WatchResult<()> {
            *self.alarm.borrow_mut() = Some(scheduled_time);
            Ok(())
        }

        async fn delete_alarm(&self) -> WatchResult<()> {
            *self.alarm.borrow_mut() = None;
            Ok(())
        }
    }

    // =========================================================
    // MockStorage 单元测试
    // =========================================================

    #[tokio::test]
    async fn test_mock_storage_put_and_get() {
        let storage = MockStorage::new();

        // 测试 put 和 get
        let value = "test_value".to_string();
        storage.put("key1", &value).await.unwrap();

        let retrieved: Option<String> = storage.get("key1").await.unwrap();
        assert_eq!(retrieved, Some(value));
    }

    #[tokio::test]
    async fn test_mock_storage_get_nonexistent() {
        let storage = MockStorage::new();

        // 获取不存在的 key
        let result: Option<String> = storage.get("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_mock_storage_delete() {
        let storage = MockStorage::new();

        // 先插入数据
        storage.put("key1", &"value1".to_string()).await.unwrap();

        // 删除存在的 key
        let deleted = storage.delete("key1").await.unwrap();
        assert!(deleted);

        // 确认已删除
        let result: Option<String> = storage.get("key1").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_mock_storage_delete_nonexistent() {
        let storage = MockStorage::new();

        // 删除不存在的 key
        let deleted = storage.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_mock_storage_overwrite() {
        let storage = MockStorage::new();

        // 插入初始值
        storage.put("key1", &"value1".to_string()).await.unwrap();

        // 覆盖写入
        storage.put("key1", &"value2".to_string()).await.unwrap();

        let retrieved: Option<String> = storage.get("key1").await.unwrap();
        assert_eq!(retrieved, Some("value2".to_string()));
    }

    #[tokio::test]
    async fn test_mock_storage_complex_struct() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct TestStruct {
            name: String,
            value: i32,
        }

        let storage = MockStorage::new();
        let data = TestStruct {
            name: "test".to_string(),
            value: 42,
        };

        storage.put("complex", &data).await.unwrap();

        let retrieved: Option<TestStruct> = storage.get("complex").await.unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn test_mock_alarm_set_and_get() {
        let storage = MockStorage::new();

        // 初始状态没有 alarm
        assert!(storage.alarm.borrow().is_none());

        // 设置 alarm
        let duration = Duration::from_secs(60);
        storage.set_alarm(duration).await.unwrap();

        assert_eq!(*storage.alarm.borrow(), Some(duration));
    }

    #[tokio::test]
    async fn test_mock_alarm_delete() {
        let storage = MockStorage::new();

        // 先设置 alarm
        storage.set_alarm(Duration::from_secs(60)).await.unwrap();

        // 删除 alarm
        storage.delete_alarm().await.unwrap();

        assert!(storage.alarm.borrow().is_none());
    }

    #[tokio::test]
    async fn test_mock_alarm_overwrite() {
        let storage = MockStorage::new();

        // 设置初始 alarm
        storage.set_alarm(Duration::from_secs(60)).await.unwrap();

        // 覆盖 alarm
        let new_duration = Duration::from_secs(120);
        storage.set_alarm(new_duration).await.unwrap();

        assert_eq!(*storage.alarm.borrow(), Some(new_duration));
    }
}
