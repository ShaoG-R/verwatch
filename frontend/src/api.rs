use gloo_net::http::Request;
use verwatch_shared::{CreateProjectRequest, DeleteTarget, ProjectConfig};

#[derive(Clone, Debug, PartialEq)]
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

    // 认证头
    fn auth_header(&self) -> (&str, &str) {
        ("X-Auth-Key", &self.secret)
    }

    /// 获取项目列表
    pub async fn get_projects(&self) -> Result<Vec<ProjectConfig>, String> {
        let url = self.url("/api/projects");
        let res = Request::get(&url)
            .header(self.auth_header().0, self.auth_header().1)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("获取项目失败: {}", res.status()));
        }

        res.json::<Vec<ProjectConfig>>()
            .await
            .map_err(|e| e.to_string())
    }

    /// 添加项目
    pub async fn add_project(&self, config: CreateProjectRequest) -> Result<ProjectConfig, String> {
        let url = self.url("/api/projects");
        let res = Request::post(&url)
            .header(self.auth_header().0, self.auth_header().1)
            .header("Content-Type", "application/json")
            .json(&config)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("添加项目失败: {}", res.status()));
        }

        res.json::<ProjectConfig>().await.map_err(|e| e.to_string())
    }

    /// 删除项目
    pub async fn delete_project(&self, id: String) -> Result<(), String> {
        let url = self.url("/api/projects");
        let target = DeleteTarget { id };
        let res = Request::delete(&url)
            .header(self.auth_header().0, self.auth_header().1)
            .header("Content-Type", "application/json")
            .json(&target)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("删除项目失败: {}", res.status()));
        }

        Ok(())
    }

    // 弹出项目（删除并返回）
    #[allow(dead_code)]
    pub async fn pop_project(&self, id: String) -> Result<Option<ProjectConfig>, String> {
        let url = self.url("/api/projects/pop");
        let target = DeleteTarget { id };
        let res = Request::delete(&url)
            .header(self.auth_header().0, self.auth_header().1)
            .header("Content-Type", "application/json")
            .json(&target)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.ok() {
            return Err(format!("弹出项目失败: {}", res.status()));
        }

        res.json::<Option<ProjectConfig>>()
            .await
            .map_err(|e| e.to_string())
    }
}
