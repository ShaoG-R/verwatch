use crate::ProjectConfig; // 引用 lib.rs 中的模型
use crate::request::{HttpClient, HttpMethod, HttpRequest};
use crate::service::GitHubRelease;
use serde::Serialize;
use serde_json::json;
use worker::{Error, Result};

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
    pub async fn send<C: HttpClient>(&self, client: &C) -> Result<()> {
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

        let resp = client.send(req).await?;

        if resp.status != 204 {
            return Err(Error::from(format!(
                "Dispatch failed with status: {}",
                resp.status
            )));
        }
        Ok(())
    }
}

// =========================================================
// 服务封装: GitHubGateway
// =========================================================

/// 封装所有与 GitHub 交互的细节
pub struct GitHubGateway<'a, C: HttpClient> {
    client: &'a C,
    global_read_token: Option<String>,
}

impl<'a, C: HttpClient> GitHubGateway<'a, C> {
    pub fn new(client: &'a C, global_read_token: Option<String>) -> Self {
        Self {
            client,
            global_read_token,
        }
    }

    pub async fn fetch_latest_release(&self, owner: &str, repo: &str) -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        );
        let mut req = HttpRequest::new(&url, HttpMethod::Get).with_header("User-Agent", USER_AGENT);

        if let Some(token) = &self.global_read_token {
            req = req.with_header("Authorization", &format!("Bearer {}", token));
        }

        let resp = self.client.send(req).await?;
        if resp.status != 200 {
            return Err(Error::from(format!(
                "Upstream API Error {}: {}",
                resp.status, url
            )));
        }
        resp.json()
    }

    pub async fn trigger_dispatch(
        &self,
        config: &ProjectConfig,
        version: &str,
        token: &str,
    ) -> Result<()> {
        let payload = json!({ "version": version });
        let event = DispatchEvent {
            owner: &config.base.my_owner,
            repo: &config.base.my_repo,
            token,
            event_type: "upstream_update",
            client_payload: payload,
        };
        event.send(self.client).await
    }
}
