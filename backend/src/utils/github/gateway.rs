use crate::error::{WatchError, WatchResult};
use crate::utils::github::release::{GitHubRelease, ReleaseTimestamp};
use crate::utils::request::{HttpClient, HttpMethod, HttpRequest};
use serde::Serialize;
use serde_json::json;
use verwatch_shared::chrono::{DateTime, Utc};
use verwatch_shared::{ComparisonMode, ProjectConfig};

pub const GITHUB_API_VERSION: &str = "2022-11-28";
const USER_AGENT: &str = "rust-watchdog-worker";

// =========================================================
// 数据结构: DispatchEvent
// =========================================================

#[derive(Serialize)]
pub struct DispatchEvent<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub token: &'a str,
    pub event_type: &'a str,
    pub client_payload: serde_json::Value,
}

impl<'a> DispatchEvent<'a> {
    pub async fn send<C: HttpClient>(&self, client: &C) -> WatchResult<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/dispatches",
            self.owner, self.repo
        );

        let body = json!({
            "event_type": self.event_type,
            "client_payload": self.client_payload
        });

        let req = HttpRequest::new(&url, HttpMethod::Post)
            .with_header("User-Agent", USER_AGENT)
            .with_header("Authorization", &format!("Bearer {}", self.token))
            .with_header("Accept", "application/vnd.github+json")
            .with_header("X-GitHub-Api-Version", GITHUB_API_VERSION)
            .with_body(body);

        let resp = client.send(req).await.map_err(|e| {
            e.in_op_with(
                "github.dispatch.send",
                format!("{}/{}", self.owner, self.repo),
            )
        })?;

        if resp.status != 204 {
            return Err(WatchError::external_api(format!(
                "Dispatch failed with status: {}",
                resp.status
            ))
            .in_op_with("github.dispatch", format!("{}/{}", self.owner, self.repo)));
        }
        Ok(())
    }
}

// =========================================================
// 2. Gateway
// =========================================================

pub struct GitHubGateway<'a, C: HttpClient> {
    client: &'a C,
    global_read_token: Option<String>,
    mode: ComparisonMode,
}

impl<'a, C: HttpClient> GitHubGateway<'a, C> {
    pub fn new(client: &'a C, global_read_token: Option<String>, mode: ComparisonMode) -> Self {
        Self {
            client,
            global_read_token,
            mode,
        }
    }

    pub async fn fetch_latest_release(
        &self,
        owner: &str,
        repo: &str,
    ) -> WatchResult<GitHubRelease> {
        let repo_path = format!("{}/{}", owner, repo);
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo_path);
        let mut req = HttpRequest::new(&url, HttpMethod::Get).with_header("User-Agent", USER_AGENT);

        if let Some(token) = &self.global_read_token {
            req = req.with_header("Authorization", &format!("Bearer {}", token));
        }

        let resp = self
            .client
            .send(req)
            .await
            .map_err(|e| e.in_op_with("github.fetch", &repo_path))?;
        if resp.status != 200 {
            return Err(WatchError::external_api(format!(
                "Upstream API Error {}: {}",
                resp.status, url
            ))
            .in_op_with("github.fetch", &repo_path));
        }

        // 手动解析 JSON Value
        let root: serde_json::Value = resp
            .json()
            .map_err(|e| e.in_op_with("github.parse", &repo_path))?;

        // 1. 获取 tag_name
        let tag_name = root
            .get("tag_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WatchError::external_api("Missing 'tag_name' in response")
                    .in_op_with("github.parse.tag", &repo_path)
            })?
            .to_string();

        // 2. 根据 mode 获取对应时间字段，如果字段不存在则报错
        let timestamp = match self.mode {
            ComparisonMode::PublishedAt => {
                let s = root
                    .get("published_at")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WatchError::external_api("Missing 'published_at' field required by config")
                            .in_op_with("github.parse.published_at", &repo_path)
                    })?;
                let t = DateTime::parse_from_rfc3339(s)
                    .map_err(|e| {
                        WatchError::external_api(format!("Invalid time format: {}", e))
                            .in_op_with("github.parse.time", &repo_path)
                    })?
                    .with_timezone(&Utc);
                ReleaseTimestamp::Published(t)
            }
            ComparisonMode::UpdatedAt => {
                let s = root
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        WatchError::external_api("Missing 'updated_at' field required by config")
                            .in_op_with("github.parse.updated_at", &repo_path)
                    })?;
                let t = DateTime::parse_from_rfc3339(s)
                    .map_err(|e| {
                        WatchError::external_api(format!("Invalid time format: {}", e))
                            .in_op_with("github.parse.time", &repo_path)
                    })?
                    .with_timezone(&Utc);
                ReleaseTimestamp::Updated(t)
            }
        };

        Ok(GitHubRelease {
            tag_name,
            timestamp,
        })
    }

    pub async fn trigger_dispatch(
        &self,
        config: &ProjectConfig,
        version: &str,
        token: &str,
    ) -> WatchResult<()> {
        let payload = json!({ "version": version });
        let event = DispatchEvent {
            owner: &config.request.base_config.my_owner,
            repo: &config.request.base_config.my_repo,
            token,
            event_type: "upstream_update",
            client_payload: payload,
        };
        event.send(self.client).await
    }
}
