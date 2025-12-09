use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::marker::PhantomData;
use worker::{Error, Result, js_sys};

// =========================================================
// 抽象接口定义
// =========================================================

pub trait MapTrait {
    type V;
    // 修改点 1: 迭代器的 Item 变为 Result<V>
    type IntoIter: Iterator<Item = Result<Self::V>>;

    fn into_values(self) -> Self::IntoIter;
}

// HashMap 实现 (Mock)
impl<T> MapTrait for HashMap<String, T> {
    type V = T;
    // 修改点 2: 使用 std::iter::Map 适配器将 T 包装为 Ok(T)
    type IntoIter =
        std::iter::Map<std::collections::hash_map::IntoValues<String, T>, fn(T) -> Result<T>>;

    fn into_values(self) -> Self::IntoIter {
        // 将普通值 T 映射为 Ok(T) 以满足接口
        self.into_values().map(Ok)
    }
}

// =========================================================
// 自定义 JS 迭代器封装
// =========================================================

pub struct JsMapValueIter<T> {
    iter: js_sys::Iterator,
    _marker: PhantomData<T>,
}

impl<T: DeserializeOwned> Iterator for JsMapValueIter<T> {
    // 修改点 3: Item 类型变为 Result<T>
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // 1. 调用 JS iterator.next()
        // 这里可能会抛出 JS 异常，我们也将其捕获为 Err
        let next_result = match self.iter.next() {
            Ok(v) => v,
            Err(e) => return Some(Err(Error::from(e))),
        };

        // 2. 检查迭代是否结束
        if next_result.done() {
            return None;
        }

        // 3. 获取 value 并尝试反序列化
        let js_val = next_result.value();

        // 修改点 4: 返回 Result
        match serde_wasm_bindgen::from_value(js_val) {
            Ok(v) => Some(Ok(v)),
            Err(e) => Some(Err(Error::from(e.to_string()))),
        }
    }
}

// Wrapper 和 StorageAdapter 实现保持结构不变，
// 只需要确保 trait bounds 匹配即可
pub struct WorkerMapWrapper<T> {
    inner: js_sys::Map,
    _marker: PhantomData<T>,
}

impl<T: DeserializeOwned> MapTrait for WorkerMapWrapper<T> {
    type V = T;
    type IntoIter = JsMapValueIter<T>;

    fn into_values(self) -> Self::IntoIter {
        let values_iter = self.inner.values();
        JsMapValueIter {
            iter: values_iter,
            _marker: PhantomData,
        }
    }
}

#[async_trait(?Send)]
pub trait StorageAdapter {
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    async fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<bool>;

    // 返回类型变更为 impl MapTrait，这里 MapTrait 内部关联类型已经变了
    async fn list_map<T: DeserializeOwned>(&self, prefix: &str) -> Result<impl MapTrait<V = T>>;
}

// =========================================================
// 生产环境实现 (WorkerStorage)
// =========================================================

pub struct WorkerStorage(pub worker::Storage);

#[async_trait(?Send)]
impl StorageAdapter for WorkerStorage {
    // ... get, put, delete 保持不变 ...
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.0.get(key).await.or_else(|e| {
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

    // 重点修改这里
    async fn list_map<T: DeserializeOwned>(&self, prefix: &str) -> Result<impl MapTrait<V = T>> {
        let opts = worker::ListOptions::new().prefix(prefix);

        // list_with_options 返回的是 Result<js_sys::Map, ...> (在 worker crate 某些版本是这样)
        // 或者它返回 HashMap。我们需要确认 worker crate 版本。
        // 假设使用的是较新的 worker-rs，它底层通常返回 JsValue，可以强转为 Map。

        let raw_map = self.0.list_with_options(opts).await?;

        // worker-rs 的 list_with_options 返回的是 js_sys::Map
        // 我们直接包装它，不进行遍历
        Ok(WorkerMapWrapper {
            inner: raw_map,
            _marker: PhantomData,
        })
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
