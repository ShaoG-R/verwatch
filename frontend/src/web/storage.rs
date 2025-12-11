//! LocalStorage 封装模块
//!
//! 使用 `web_sys::Storage` 替代 `gloo-storage`，提供简洁的本地存储接口。

/// 本地存储操作封装
///
/// 提供静态方法访问浏览器 LocalStorage API。
pub struct LocalStorage;

impl LocalStorage {
    /// 获取 LocalStorage 实例
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    /// 获取存储的字符串值
    ///
    /// # 返回
    /// - `Some(String)` 如果键存在且有值
    /// - `None` 如果键不存在或发生错误
    pub fn get(key: &str) -> Option<String> {
        Self::storage()?.get_item(key).ok()?
    }

    /// 设置存储值
    ///
    /// # 参数
    /// - `key`: 存储键
    /// - `value`: 要存储的值
    ///
    /// # 返回
    /// - `true` 如果操作成功
    /// - `false` 如果操作失败
    pub fn set(key: &str, value: &str) -> bool {
        Self::storage()
            .and_then(|s| s.set_item(key, value).ok())
            .is_some()
    }

    /// 删除存储的键值对
    ///
    /// # 参数
    /// - `key`: 要删除的键
    ///
    /// # 返回
    /// - `true` 如果操作成功
    /// - `false` 如果操作失败
    pub fn delete(key: &str) -> bool {
        Self::storage()
            .and_then(|s| s.remove_item(key).ok())
            .is_some()
    }
}
