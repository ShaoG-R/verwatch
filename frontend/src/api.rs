use crate::web::HttpClient;
use serde::{Deserialize, Serialize};

use verwatch_shared::{
    CreateProjectRequest, DeleteTarget, ProjectConfig,
    protocol::{PopProjectRequest, SwitchMonitorRequest, TriggerCheckRequest},
};

// 辅助函数：序列化 JSON
fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json_wasm::to_string(value).map_err(|e| e.to_string())
}

// 辅助函数：反序列化 JSON
fn from_json<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, String> {
    serde_json_wasm::from_str(text).map_err(|e| e.to_string())
}

#[derive(Clone, PartialEq)]
pub struct VerWatchApi {
    pub base_url: String,
    pub secret: String,
}

impl VerWatchApi {
    pub fn new(base_url: String, secret: String) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self { base_url, secret }
    }

    fn url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }

    /// 获取项目列表
    pub async fn get_projects(&self) -> Result<Vec<ProjectConfig>, String> {
        let url = self.url("/api/projects");
        let res = HttpClient::get(&url)
            .header("X-Auth-Key", &self.secret)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("获取项目失败: {}", res.status()));
        }

        let text = res.text().await.map_err(|e| e.to_string())?;
        from_json(&text)
    }

    /// 添加项目
    pub async fn add_project(&self, config: CreateProjectRequest) -> Result<ProjectConfig, String> {
        let url = self.url("/api/projects");
        let body = to_json(&config)?;
        let res = HttpClient::post(&url)
            .header("X-Auth-Key", &self.secret)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("添加项目失败: {}", res.status()));
        }

        let text = res.text().await.map_err(|e| e.to_string())?;
        from_json(&text)
    }

    /// 删除项目
    pub async fn delete_project(&self, id: String) -> Result<bool, String> {
        let url = self.url("/api/projects");
        let target = DeleteTarget { id };
        let body = to_json(&target)?;
        let res = HttpClient::delete(&url)
            .header("X-Auth-Key", &self.secret)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        match res.status() {
            204 => Ok(true),
            404 => Ok(false),
            _ => Err(format!("删除项目失败: {}", res.status())),
        }
    }

    // 弹出项目（删除并返回）
    #[allow(dead_code)]
    pub async fn pop_project(&self, id: String) -> Result<Option<ProjectConfig>, String> {
        let url = self.url("/api/projects/pop");
        let target = PopProjectRequest { id };
        let body = to_json(&target)?;
        let res = HttpClient::delete(&url)
            .header("X-Auth-Key", &self.secret)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("弹出项目失败: {}", res.status()));
        }

        let text = res.text().await.map_err(|e| e.to_string())?;
        from_json(&text)
    }

    /// 切换监控状态 (Start/Stop)
    pub async fn switch_monitor(&self, unique_key: String, paused: bool) -> Result<bool, String> {
        let url = self.url("/api/projects/switch");
        let payload = SwitchMonitorRequest { unique_key, paused };
        let body = to_json(&payload)?;
        let res = HttpClient::post(&url)
            .header("X-Auth-Key", &self.secret)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("切换状态失败: {}", res.status()));
        }

        let text = res.text().await.map_err(|e| e.to_string())?;
        from_json(&text)
    }

    /// 触发立即检查
    pub async fn trigger_check(&self, unique_key: String) -> Result<(), String> {
        let url = self.url("/api/projects/trigger");
        let payload = TriggerCheckRequest { unique_key };
        let body = to_json(&payload)?;
        let res = HttpClient::post(&url)
            .header("X-Auth-Key", &self.secret)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("触发检查失败: {}", res.status()));
        }

        Ok(())
    }
}
