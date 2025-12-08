use crate::request::{HttpClient, HttpMethod, HttpRequest};
use serde::Serialize;
use serde_json::json;
use worker::{Error, Result};

pub const GITHUB_API_VERSION: &str = "2022-11-28";

// =========================================================
// 业务逻辑: DispatchEvent
// =========================================================

#[derive(Serialize)]
pub struct DispatchEvent<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub token: &'a str,
    pub event_type: &'a str,
    pub client_payload: serde_json::Value,
}

// https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#create-a-repository-dispatch-event
//
// Request example
// curl -L \
//   -X POST \
//   -H "Accept: application/vnd.github+json" \
//   -H "Authorization: Bearer <YOUR-TOKEN>" \
//   -H "X-GitHub-Api-Version: 2022-11-28" \
//   https://api.github.com/repos/OWNER/REPO/dispatches \
//   -d '{"event_type":"on-demand-test","client_payload":{"unit":false,"integration":true}}'
//
// Response
// Status: 204
impl<'a> DispatchEvent<'a> {
    // 这里接受任何实现了 HttpClient 的客户端
    // 从而解耦了具体的 HTTP 实现
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
            .with_header("User-Agent", "rust-cf-worker")
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
