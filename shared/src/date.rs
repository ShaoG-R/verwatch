//! 时间类型模块
//!
//! 提供两种时间类型：
//! - `Timestamp`: 可序列化的毫秒时间戳，用于传输和存储
//! - `Date`: 操作型时间类型，提供 now(), parse() 等方法

use crate::DurationSecs;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Sub};
use std::time::Duration;

// =========================================================
// Timestamp - 可传输的时间戳类型
// =========================================================

/// 毫秒时间戳，用于序列化传输和存储
///
/// 内部存储为 `i64`，表示自 Unix 纪元以来的毫秒数
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// 创建新的时间戳
    #[inline]
    pub const fn new(ms: i64) -> Self {
        Self(ms)
    }

    /// 获取毫秒值
    #[inline]
    pub const fn as_millis(&self) -> i64 {
        self.0
    }

    /// 获取秒值
    #[inline]
    pub const fn as_secs(&self) -> i64 {
        self.0 / 1000
    }

    /// 转换为整数毫秒（用于 key 生成等场景）
    #[inline]
    pub const fn as_millis_i64(&self) -> i64 {
        self.0
    }
}

impl From<i64> for Timestamp {
    fn from(ms: i64) -> Self {
        Self(ms)
    }
}

impl From<Timestamp> for i64 {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

impl Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs.as_millis() as i64)
    }
}

impl Add<DurationSecs> for Timestamp {
    type Output = Self;

    fn add(self, rhs: DurationSecs) -> Self::Output {
        Self(self.0 + rhs.as_millis() as i64)
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = Duration;

    /// 计算两个时间戳之间的差值（返回 Duration）
    fn sub(self, rhs: Timestamp) -> Self::Output {
        let diff_ms = (self.0 - rhs.0).max(0);
        Duration::from_millis(diff_ms as u64)
    }
}

// =========================================================
// Date - 操作型时间类型
// =========================================================

/// 操作型时间类型，封装 js_sys::Date
///
/// 用于获取当前时间、解析时间字符串等操作
#[derive(Debug, Clone)]
pub struct Date(js_sys::Date);

impl Date {
    /// 获取当前时间
    #[inline]
    pub fn now() -> Self {
        Self(js_sys::Date::new_0())
    }

    /// 获取当前时间的毫秒时间戳
    #[inline]
    pub fn now_timestamp() -> Timestamp {
        Timestamp(js_sys::Date::now() as i64)
    }

    /// 从毫秒时间戳创建
    #[inline]
    pub fn from_timestamp(ts: Timestamp) -> Self {
        Self(js_sys::Date::new(&(ts.0 as f64).into()))
    }

    /// 从 ISO 8601 / RFC 3339 字符串解析
    ///
    /// 返回 None 如果解析失败
    pub fn parse(s: &str) -> Option<Self> {
        let ms = js_sys::Date::parse(s);
        if ms.is_nan() {
            None
        } else {
            Some(Self(js_sys::Date::new(&ms.into())))
        }
    }

    /// 解析字符串并直接返回时间戳
    ///
    /// 返回 None 如果解析失败
    pub fn parse_timestamp(s: &str) -> Option<Timestamp> {
        let ms = js_sys::Date::parse(s);
        if ms.is_nan() {
            None
        } else {
            Some(Timestamp(ms as i64))
        }
    }

    /// 转换为时间戳
    #[inline]
    pub fn timestamp(&self) -> Timestamp {
        Timestamp(self.0.get_time() as i64)
    }

    /// 获取毫秒值
    #[inline]
    pub fn as_millis(&self) -> i64 {
        self.0.get_time() as i64
    }
}

impl From<Timestamp> for Date {
    fn from(ts: Timestamp) -> Self {
        Self::from_timestamp(ts)
    }
}

impl From<Date> for Timestamp {
    fn from(date: Date) -> Self {
        date.timestamp()
    }
}

impl From<&Date> for Timestamp {
    fn from(date: &Date) -> Self {
        date.timestamp()
    }
}
