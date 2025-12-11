use worker::*;

pub mod error;
pub mod logic;
mod project;
mod repository;

pub(crate) mod utils {
    pub mod github;
    pub mod request;
    pub mod rpc;
}

use error::WatchError;
use logic::AdminLogic;
use repository::DoProjectRegistry;
use verwatch_shared::{
    CreateProjectRequest, DeleteTarget, HEADER_AUTH_KEY,
    protocol::{PopProjectRequest, SwitchMonitorRequest, TriggerCheckRequest},
};

// =========================================================
// 常量定义
// =========================================================
const DEFAULT_REGISTRY_BINDING: &str = "PROJECT_REGISTRY";
const DEFAULT_SECRET_VAR_NAME: &str = "ADMIN_SECRET";

// =========================================================
// 宏定义 (包含日志和响应处理)
// =========================================================

#[cfg(target_arch = "wasm32")]
macro_rules! log_error { ($($t:tt)*) => (worker::console_error!($($t)*)) }
#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_error { ($($t:tt)*) => (eprintln!($($t)*)) }

// 辅助函数：将 WatchError 映射为 Worker Response
fn map_error_to_response(e: WatchError) -> worker::Response {
    let status = e.status_code();
    let msg = e.to_string();

    // 对于 5xx 错误，记录日志以便排查
    if status >= 500 {
        log_error!("Internal Error [{}]: {}", e.error_code(), msg);
        return Response::error("Internal Server Error", status).unwrap();
    }

    // 对于 4xx 错误，直接返回具体错误信息给客户端
    Response::error(msg, status).unwrap()
}

/// 由于 Worker 需要 `fn(Request, RouteContext<()>) -> Result<Response>`
/// 但我们希望在 Controller 中统一返回 `WatchResult<Response>` 并在外层统一处理 Error
/// 所以使用此宏生成 wrapper 函数
macro_rules! console_handler {
    ($wrapper_name:ident, $impl_name:ident, $op:expr) => {
        async fn $wrapper_name(req: Request, ctx: RouteContext<()>) -> Result<Response> {
            match $impl_name(req, ctx).await {
                Ok(res) => Ok(res),
                Err(e) => Ok(map_error_to_response(e.in_op($op))),
            }
        }
    };
}

// =========================================================
// 运行时配置与鉴权
// =========================================================

struct RuntimeConfig {
    registry_binding: String,
    admin_secret_name: String,
}

impl RuntimeConfig {
    fn new(env: &Env) -> Self {
        Self {
            registry_binding: env
                .var("REGISTRY_BINDING")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_REGISTRY_BINDING.to_string()),
            admin_secret_name: env
                .var("ADMIN_SECRET_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_SECRET_VAR_NAME.to_string()),
        }
    }
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0, |acc, (&x, &y)| acc | (x ^ y))
        == 0
}

fn ensure_admin_auth(req: &Request, env: &Env, config: &RuntimeConfig) -> error::WatchResult<()> {
    let auth_header = req
        .headers()
        .get(HEADER_AUTH_KEY)
        .map_err(|e| WatchError::invalid_input(e.to_string()).in_op("auth.header"))?
        .unwrap_or_default();
    let secret = env
        .secret(&config.admin_secret_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if secret.is_empty() || !constant_time_eq(&auth_header, &secret) {
        return Err(WatchError::unauthorized("Invalid Secret").in_op("auth.verify"));
    }
    Ok(())
}

// =========================================================
// API Controllers (适配层)
// =========================================================

async fn list_projects(req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.list_projects().await?;

    Response::from_json(&result).map_err(|e| WatchError::serialization(e.to_string()))
}

async fn create_project(mut req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let req_data: CreateProjectRequest = req
        .json()
        .await
        .map_err(|e| WatchError::serialization(format!("Invalid JSON Body: {}", e)))?;

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.create_project(req_data).await?;

    Response::from_json(&result).map_err(|e| WatchError::serialization(e.to_string()))
}

async fn delete_project(mut req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let target: DeleteTarget = req
        .json()
        .await
        .map_err(|e| WatchError::serialization(format!("Invalid JSON Body: {}", e)))?;

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.delete_project(target).await?;

    match result {
        true => Response::empty()
            .map(|r| r.with_status(204))
            .map_err(|e| WatchError::store(e.to_string())),
        false => Err(WatchError::not_found("Project not found")),
    }
}

async fn pop_project(mut req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let req_data: PopProjectRequest = req
        .json()
        .await
        .map_err(|e| WatchError::serialization(format!("Invalid JSON Body: {}", e)))?;
    let target = DeleteTarget { id: req_data.id };

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.pop_project(target).await?;

    Response::from_json(&result).map_err(|e| WatchError::serialization(e.to_string()))
}

async fn switch_monitor(mut req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let cmd: SwitchMonitorRequest = req
        .json()
        .await
        .map_err(|e| WatchError::serialization(format!("Invalid JSON Body: {}", e)))?;

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.switch_monitor(cmd.unique_key, cmd.paused).await?;

    Response::from_json(&result).map_err(|e| WatchError::serialization(e.to_string()))
}

async fn trigger_check(mut req: Request, ctx: RouteContext<()>) -> error::WatchResult<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    ensure_admin_auth(&req, &ctx.env, &cfg)?;

    let cmd: TriggerCheckRequest = req
        .json()
        .await
        .map_err(|e| WatchError::serialization(format!("Invalid JSON Body: {}", e)))?;

    let registry = DoProjectRegistry::new(&ctx.env, &cfg.registry_binding)
        .map_err(|e| WatchError::store(e.to_string()))?;

    let logic = AdminLogic::new(&registry);
    let result = logic.trigger_check(cmd.unique_key).await?;

    Response::from_json(&result).map_err(|e| WatchError::serialization(e.to_string()))
}

// =========================================================
// Entry Points
// =========================================================

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let cors = Cors::new()
        .with_origins(vec!["*"])
        .with_methods(vec![
            Method::Get,
            Method::Post,
            Method::Delete,
            Method::Options,
        ])
        .with_allowed_headers(vec!["Content-Type", HEADER_AUTH_KEY]);

    console_handler!(list_projects_handler, list_projects, "project.list");
    console_handler!(create_project_handler, create_project, "project.create");
    console_handler!(delete_project_handler, delete_project, "project.delete");
    console_handler!(pop_project_handler, pop_project, "project.pop");
    console_handler!(switch_monitor_handler, switch_monitor, "project.switch");
    console_handler!(trigger_check_handler, trigger_check, "project.trigger");

    let router = Router::new();
    router
        .get_async("/api/projects", list_projects_handler)
        .post_async("/api/projects", create_project_handler)
        .delete_async("/api/projects", delete_project_handler)
        .delete_async("/api/projects/pop", pop_project_handler)
        .post_async("/api/projects/switch", switch_monitor_handler)
        .post_async("/api/projects/trigger", trigger_check_handler)
        .options_async("/api/projects", |_, _| async { Response::empty() })
        .options_async("/api/projects/pop", |_, _| async { Response::empty() })
        .options_async("/api/projects/switch", |_, _| async { Response::empty() })
        .options_async("/api/projects/trigger", |_, _| async { Response::empty() })
        .run(req, env)
        .await?
        .with_cors(&cors)
}
