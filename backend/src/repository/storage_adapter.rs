use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use worker::{Error, Result}; // 引入 Error

// =========================================================
// 抽象接口定义
// =========================================================

pub trait MapTrait {
    type V;
    fn values(&self) -> impl Iterator<Item = &Self::V>;
}

// HashMap 天然满足 MapTrait，因为它拥有数据的所有权，可以分发引用
impl<T> MapTrait for HashMap<String, T> {
    type V = T;
    fn values(&self) -> impl Iterator<Item = &Self::V> {
        self.values()
    }
}

#[async_trait(?Send)]
pub trait StorageAdapter {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<bool>;
    // 返回值使用 impl MapTrait，这允许我们在实现中隐藏具体的容器类型 (HashMap)
    async fn list_map<T: DeserializeOwned>(&self, prefix: &str) -> Result<impl MapTrait<V = T>>;
}

// =========================================================
// 生产环境实现 (WorkerStorage)
// =========================================================

pub struct WorkerStorage(pub worker::Storage);

#[async_trait(?Send)]
impl StorageAdapter for WorkerStorage {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.0.get(key).await.or_else(|e| {
            // 某些版本的 worker crate 在 key 不存在时会报错，这里做一下兼容
            if e.to_string().contains("No such value") {
                Ok(None)
            } else {
                Err(e)
            }
        })
    }

    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.0.put(key, value).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.0.delete(key).await
    }

    async fn list_map<T: DeserializeOwned>(&self, prefix: &str) -> Result<impl MapTrait<V = T>> {
        // 1. 调用 worker 原生 API，获取 js_sys::Map
        let opts = worker::ListOptions::new().prefix(prefix);
        let raw_map = self.0.list_with_options(opts).await?;

        // 2. 转换为 Rust HashMap
        let mut result = HashMap::new();

        // FIX(E0599, E0308): raw_map.keys() 返回的是 Result<JsValue, ...> 的迭代器
        for key_res in raw_map.keys() {
            // 解包 Iterator 返回的 Result
            let key_js = key_res.map_err(|_| Error::from("JS Iterator Error"))?;

            // 确保 key 是 String
            let key_str = key_js
                .as_string()
                .ok_or_else(|| Error::from("Key is not string"))?;

            // 获取 Value (JsValue)
            let val_js = raw_map.get(&key_js);

            // 使用 serde_wasm_bindgen 进行反序列化，并转换错误类型
            let val: T =
                serde_wasm_bindgen::from_value(val_js).map_err(|e| Error::from(e.to_string()))?;

            result.insert(key_str, val);
        }

        Ok(result)
    }
}

// =========================================================
// 测试环境实现 (MockStorage)
// =========================================================

#[cfg(test)]
pub struct MockStorage {
    // 存储序列化后的 JSON 字符串，模拟真实存储的序列化边界
    pub map: std::cell::RefCell<HashMap<String, String>>,
}

#[cfg(test)]
impl MockStorage {
    pub fn new() -> Self {
        Self {
            map: std::cell::RefCell::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
#[async_trait(?Send)]
impl StorageAdapter for MockStorage {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let map = self.map.borrow();
        if let Some(val_str) = map.get(key) {
            let val = serde_json::from_str(val_str)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let val_str = serde_json::to_string(value)?;
        self.map.borrow_mut().insert(key.to_string(), val_str);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        Ok(self.map.borrow_mut().remove(key).is_some())
    }

    async fn list_map<T: DeserializeOwned>(&self, prefix: &str) -> Result<impl MapTrait<V = T>> {
        let map = self.map.borrow();
        let mut result = HashMap::new();

        for (k, v_str) in map.iter() {
            if k.starts_with(prefix) {
                let val: T = serde_json::from_str(v_str)?;
                result.insert(k.clone(), val);
            }
        }
        Ok(result)
    }
}
