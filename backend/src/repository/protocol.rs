use serde::{Deserialize, Serialize, de::DeserializeOwned};
use verwatch_shared::ProjectConfig;
use worker::Method;

/// 定义请求与响应的绑定关系
pub trait ApiRequest: Serialize + DeserializeOwned {
    /// 该请求对应的响应类型
    type Response: Serialize + DeserializeOwned;
    /// DO 内部路由路径
    const PATH: &'static str;
    /// 建议的 HTTP 方法
    const METHOD: Method = Method::Post;
}

// =========================================================
// Registry 指令定义
// =========================================================

/// 注册一个 ProjectMonitor
/// 接收完整的 ProjectConfig，内部计算 unique_key 并调用 Monitor setup
#[derive(Serialize, Deserialize)]
pub struct RegisterMonitorCmd {
    pub config: ProjectConfig,
}

impl ApiRequest for RegisterMonitorCmd {
    type Response = String; // 返回 unique_key
    const PATH: &'static str = "/registry/register";
    const METHOD: Method = Method::Post;
}

/// 注销一个 ProjectMonitor
/// 内部调用 Monitor stop
#[derive(Serialize, Deserialize)]
pub struct UnregisterMonitorCmd {
    pub unique_key: String,
}

impl ApiRequest for UnregisterMonitorCmd {
    type Response = bool;
    const PATH: &'static str = "/registry/unregister";
    const METHOD: Method = Method::Delete;
}

/// 获取所有已注册的 Monitor 的 ProjectConfig 列表
/// 会遍历查询每个 Monitor
#[derive(Serialize, Deserialize)]
pub struct ListMonitorsCmd;

impl ApiRequest for ListMonitorsCmd {
    type Response = Vec<ProjectConfig>;
    const PATH: &'static str = "/registry/list";
    const METHOD: Method = Method::Get;
}

/// 检查某个 Monitor 是否已注册
#[derive(Serialize, Deserialize)]
pub struct IsRegisteredCmd {
    pub unique_key: String,
}

impl ApiRequest for IsRegisteredCmd {
    type Response = bool;
    const PATH: &'static str = "/registry/exists";
    const METHOD: Method = Method::Get;
}

/// 切换 Monitor 监控状态 (Start/Stop)
#[derive(Serialize, Deserialize)]
pub struct RegistrySwitchMonitorCmd {
    pub unique_key: String,
    pub paused: bool,
}

impl ApiRequest for RegistrySwitchMonitorCmd {
    type Response = bool;
    const PATH: &'static str = "/registry/switch";
    const METHOD: Method = Method::Post;
}

/// 手动触发 Monitor 检查
#[derive(Serialize, Deserialize)]
pub struct RegistryTriggerCheckCmd {
    pub unique_key: String,
}

impl ApiRequest for RegistryTriggerCheckCmd {
    type Response = bool; // 指示触发命令是否成功发送
    const PATH: &'static str = "/registry/trigger";
    const METHOD: Method = Method::Post;
}
