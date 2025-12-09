use serde::{Deserialize, Serialize, de::DeserializeOwned};
use verwatch_shared::ProjectConfig;
use worker::Method;

/// 定义请求与响应的绑定关系
pub trait ApiRequest: Serialize + DeserializeOwned {
    /// 该请求对应的响应类型
    type Response: Serialize + DeserializeOwned;
    /// DO 内部路由路径
    const PATH: &'static str;
    /// 建议的 HTTP 方法（通常统一用 POST 简化传参，也可以保留语义）
    const METHOD: Method = Method::Post;
}

// --- 具体请求定义 ---

// 1. 获取项目列表
#[derive(Serialize, Deserialize)]
pub struct ListProjectsCmd {
    pub prefix: Option<String>,
}
impl ApiRequest for ListProjectsCmd {
    type Response = Vec<ProjectConfig>;
    const PATH: &'static str = "/rpc/projects/list";
}

// 2. 批量获取项目+状态 (优化版)
#[derive(Serialize, Deserialize)]
pub struct ListProjectsWithStatesCmd;
impl ApiRequest for ListProjectsWithStatesCmd {
    type Response = Vec<(ProjectConfig, Option<String>)>;
    const PATH: &'static str = "/rpc/projects/list_with_states";
}

// 3. 获取单个项目
#[derive(Serialize, Deserialize)]
pub struct GetProjectCmd {
    pub id: String,
}
impl ApiRequest for GetProjectCmd {
    type Response = Option<ProjectConfig>;
    const PATH: &'static str = "/rpc/projects/get";
}

// 4. 保存项目
#[derive(Serialize, Deserialize)]
pub struct SaveProjectCmd {
    pub config: ProjectConfig,
}
impl ApiRequest for SaveProjectCmd {
    type Response = (); // 无返回值，或返回 String 消息
    const PATH: &'static str = "/rpc/projects/save";
}

// 5. 删除项目
#[derive(Serialize, Deserialize)]
pub struct DeleteProjectCmd {
    pub id: String,
}
impl ApiRequest for DeleteProjectCmd {
    type Response = bool;
    const PATH: &'static str = "/rpc/projects/delete";
}

// 6. 切换暂停状态 (原子操作)
#[derive(Serialize, Deserialize)]
pub struct TogglePauseCmd {
    pub id: String,
}
impl ApiRequest for TogglePauseCmd {
    type Response = bool; // 返回新的状态
    const PATH: &'static str = "/rpc/projects/toggle";
    const METHOD: Method = Method::Patch; // 可以保留语义，也可以统一 POST
}

// 7. 状态管理 (KV)
#[derive(Serialize, Deserialize)]
pub struct GetVersionStateCmd {
    pub key: String,
}
impl ApiRequest for GetVersionStateCmd {
    type Response = Option<String>;
    const PATH: &'static str = "/rpc/state/get";
}

#[derive(Serialize, Deserialize)]
pub struct SetVersionStateCmd {
    pub key: String,
    pub value: String,
}
impl ApiRequest for SetVersionStateCmd {
    type Response = ();
    const PATH: &'static str = "/rpc/state/set";
}
