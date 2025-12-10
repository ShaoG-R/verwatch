use crate::{CreateProjectRequest, DeleteTarget, ProjectConfig};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// HTTP Methods for API Requests
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch, // Added Patch just in case
}

/// A trait that defines the request-response relationship and metadata for an API endpoint.
pub trait ApiRequest: Serialize + DeserializeOwned {
    /// The response type returned by this request.
    type Response: Serialize + DeserializeOwned;
    /// The URL path (or suffix).
    const PATH: &'static str;
    /// The HTTP method.
    const METHOD: HttpMethod;
}

// =========================================================
// Request Definitions
// =========================================================

/// List all projects
#[derive(Debug, Serialize, Deserialize)]
pub struct ListProjectsRequest;

impl ApiRequest for ListProjectsRequest {
    type Response = Vec<ProjectConfig>;
    const PATH: &'static str = "/api/projects";
    const METHOD: HttpMethod = HttpMethod::Get;
}

/// Create a new project (Wraps logic, re-uses CreateProjectRequest)
// Note: CreateProjectRequest is defined in lib.rs
impl ApiRequest for CreateProjectRequest {
    type Response = ProjectConfig;
    const PATH: &'static str = "/api/projects";
    const METHOD: HttpMethod = HttpMethod::Post;
}

/// Delete a project
/// We create a specific request struct for better clarity,
/// but the backend currently expects DeleteTarget.
/// We can implement ApiRequest for DeleteTarget.
impl ApiRequest for DeleteTarget {
    type Response = (); // 204 or 404. We'll Treat success as ().
    const PATH: &'static str = "/api/projects";
    const METHOD: HttpMethod = HttpMethod::Delete;
}

/// Pop a project (Delete and return config)
#[derive(Debug, Serialize, Deserialize)]
pub struct PopProjectRequest {
    pub id: String,
}

impl ApiRequest for PopProjectRequest {
    type Response = Option<ProjectConfig>;
    const PATH: &'static str = "/api/projects/pop";
    const METHOD: HttpMethod = HttpMethod::Delete;
}

impl From<DeleteTarget> for PopProjectRequest {
    fn from(target: DeleteTarget) -> Self {
        Self { id: target.id }
    }
}

/// Switch monitor state
#[derive(Debug, Serialize, Deserialize)]
pub struct SwitchMonitorRequest {
    pub unique_key: String,
    pub paused: bool,
}

impl ApiRequest for SwitchMonitorRequest {
    type Response = bool;
    const PATH: &'static str = "/api/projects/switch";
    const METHOD: HttpMethod = HttpMethod::Post;
}

/// Trigger a check manually
#[derive(Debug, Serialize, Deserialize)]
pub struct TriggerCheckRequest {
    pub unique_key: String,
}

impl ApiRequest for TriggerCheckRequest {
    type Response = ();
    const PATH: &'static str = "/api/projects/trigger";
    const METHOD: HttpMethod = HttpMethod::Post;
}
