use serde::{Deserialize, Serialize, de::DeserializeOwned};
use verwatch_shared::ProjectConfig;
use worker::Method;

pub trait ApiRequest: Serialize + DeserializeOwned {
    type Response: Serialize + DeserializeOwned;
    const PATH: &'static str;
    const METHOD: Method = Method::Post;
}

// =========================================================
// 指令定义
// =========================================================

/// 接收 Config，保存并等待 initial_delay 时间后触发第一次 Alarm
#[derive(Serialize, Deserialize)]
pub struct SetupMonitorCmd {
    pub config: ProjectConfig,
}

impl ApiRequest for SetupMonitorCmd {
    type Response = ();
    const PATH: &'static str = "/monitor/setup";
    const METHOD: Method = Method::Post;
}

/// 停止监控 (Stop)
/// 清除所有状态和 Alarm
#[derive(Serialize, Deserialize)]
pub struct StopMonitorCmd;

impl ApiRequest for StopMonitorCmd {
    type Response = ();
    const PATH: &'static str = "/monitor/stop";
    const METHOD: Method = Method::Delete;
}

/// 手动触发检查 (Trigger)
/// 不等待 Alarm，立即运行一次检查逻辑
#[derive(Serialize, Deserialize)]
pub struct TriggerCheckCmd;

impl ApiRequest for TriggerCheckCmd {
    type Response = ();
    const PATH: &'static str = "/monitor/trigger";
    const METHOD: Method = Method::Post;
}

/// 获取当前配置
#[derive(Serialize, Deserialize)]
pub struct GetConfigCmd;

impl ApiRequest for GetConfigCmd {
    type Response = Option<ProjectConfig>;
    const PATH: &'static str = "/monitor/config";
    const METHOD: Method = Method::Get;
}

/// 切换监控启停状态
#[derive(Serialize, Deserialize)]
pub struct SwitchMonitorCmd {
    pub paused: bool,
}

impl ApiRequest for SwitchMonitorCmd {
    type Response = ();
    const PATH: &'static str = "/monitor/switch";
    const METHOD: Method = Method::Post;
}
