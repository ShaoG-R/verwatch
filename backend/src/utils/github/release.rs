use crate::error::{WatchError, WatchResult};
use serde::{Deserialize, Serialize};
use verwatch_shared::Timestamp;

// =========================================================
// 1. Enum & Struct
// =========================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ReleaseTimestamp {
    Published(Timestamp),
    Updated(Timestamp),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub timestamp: ReleaseTimestamp,
}

impl GitHubRelease {
    /// 判断当前 release (self) 是否比已存在的 release (current) 更新。
    ///
    /// # 错误
    /// 如果两者的比较模式不匹配（例如一个是 Published 另一个是 Updated），
    /// 则返回 Err。
    pub fn is_newer_than(&self, current: &GitHubRelease) -> WatchResult<bool> {
        match (self.timestamp, current.timestamp) {
            // 只有同类型才能比较
            (ReleaseTimestamp::Published(t_new), ReleaseTimestamp::Published(t_old)) => {
                Ok(t_new > t_old)
            }
            (ReleaseTimestamp::Updated(t_new), ReleaseTimestamp::Updated(t_old)) => {
                Ok(t_new > t_old)
            }
            // 类型不匹配，视为逻辑错误（可能是配置被修改了，或者数据脏了）
            _ => Err(WatchError::invalid_input(format!(
                "Comparison mode mismatch: New is {:?}, but Current is {:?}",
                self.timestamp, current.timestamp
            ))
            .in_op("release.compare")),
        }
    }
}
