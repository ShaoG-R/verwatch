//! 表单状态管理模块
//!
//! 将零散的 signal 整合为 `FormState` 结构体，负责：
//! - 数据的持有
//! - 数据的重置
//! - 数据到请求对象的转换

use leptos::prelude::*;
use verwatch_shared::{BaseConfig, ComparisonMode, CreateProjectRequest, DurationSecs, TimeConfig};

/// 表单状态结构体
///
/// 使用 `RwSignal` 因为它实现了 `Copy` trait，非常适合作为 Props 在组件间传递。
#[derive(Clone, Copy)]
pub struct FormState {
    // 基础信息
    pub u_owner: RwSignal<String>,
    pub u_repo: RwSignal<String>,
    pub m_owner: RwSignal<String>,
    pub m_repo: RwSignal<String>,
    pub comp_mode: RwSignal<ComparisonMode>,
    pub token_secret: RwSignal<String>,

    // 时间配置
    pub use_custom_time: RwSignal<bool>,
    pub check_interval_val: RwSignal<u64>,
    pub check_interval_unit: RwSignal<String>,
    pub retry_interval_seconds: RwSignal<u64>,
}

impl FormState {
    /// 创建新的表单状态，所有字段使用默认值
    pub fn new() -> Self {
        Self {
            u_owner: RwSignal::new(String::new()),
            u_repo: RwSignal::new(String::new()),
            m_owner: RwSignal::new(String::new()),
            m_repo: RwSignal::new(String::new()),
            comp_mode: RwSignal::new(ComparisonMode::PublishedAt),
            token_secret: RwSignal::new(String::new()),
            use_custom_time: RwSignal::new(false),
            check_interval_val: RwSignal::new(1),
            check_interval_unit: RwSignal::new("hours".to_string()),
            retry_interval_seconds: RwSignal::new(10),
        }
    }

    /// 重置表单到初始状态
    pub fn reset(&self) {
        self.u_owner.set(String::new());
        self.u_repo.set(String::new());
        self.m_owner.set(String::new());
        self.m_repo.set(String::new());
        self.comp_mode.set(ComparisonMode::PublishedAt);
        self.token_secret.set(String::new());
        self.use_custom_time.set(false);
        self.check_interval_val.set(1);
        self.check_interval_unit.set("hours".to_string());
        self.retry_interval_seconds.set(10);
    }

    /// 将表单状态转换为 API 请求对象
    pub fn to_request(&self) -> CreateProjectRequest {
        let secret = self.token_secret.get();
        let secret_opt = if secret.trim().is_empty() {
            None
        } else {
            Some(secret)
        };

        let time_config = if self.use_custom_time.get() {
            let multiplier = if self.check_interval_unit.get() == "minutes" {
                60
            } else {
                3600
            };
            TimeConfig {
                check_interval: DurationSecs::from_secs(self.check_interval_val.get() * multiplier),
                retry_interval: DurationSecs::from_secs(self.retry_interval_seconds.get()),
            }
        } else {
            TimeConfig::default()
        };

        CreateProjectRequest {
            base_config: BaseConfig {
                upstream_owner: self.u_owner.get(),
                upstream_repo: self.u_repo.get(),
                my_owner: self.m_owner.get(),
                my_repo: self.m_repo.get(),
            },
            time_config,
            initial_delay: DurationSecs::from_secs(0),
            comparison_mode: self.comp_mode.get(),
            dispatch_token_secret: secret_opt,
        }
    }
}

impl Default for FormState {
    fn default() -> Self {
        Self::new()
    }
}
