use worker::*;

pub mod error;
pub mod logic;
mod project;
mod repository;

pub(crate) mod utils {
    pub mod github;
    pub mod request;
}

use error::AppError;
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

// 辅助函数：将 AppError 映射为 Worker Response
fn map_error_to_response(e: AppError) -> worker::Response {
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

// 统一响应宏
macro_rules! respond {
    (json, $expr:expr) => {
        match $expr {
            Ok(v) => Response::from_json(&v),
            Err(e) => Ok(map_error_to_response(e)),
        }
    };
}

// 辅助宏：处理 Option/Result 类型的 unwrapping
macro_rules! unwrap_or_resp {
    ($expr:expr, $err_mapper:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Ok(map_error_to_response($err_mapper(e.to_string()))),
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

fn ensure_admin_auth(req: &Request, env: &Env, config: &RuntimeConfig) -> error::Result<()> {
    let auth_header = req
        .headers()
        .get(HEADER_AUTH_KEY)
        .map_err(|e| AppError::InvalidInput(e.to_string()))?
        .unwrap_or_default();
    let secret = env
        .secret(&config.admin_secret_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    if secret.is_empty() || !constant_time_eq(&auth_header, &secret) {
        return Err(AppError::Unauthorized("Invalid Secret".into()));
    }
    Ok(())
}

// =========================================================
// API Controllers (适配层)
// =========================================================

async fn list_projects(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.list_projects().await;

    respond!(json, result)
}

async fn create_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let req_data: CreateProjectRequest =
        unwrap_or_resp!(req.json().await, |e| AppError::Serialization(format!(
            "Invalid JSON Body: {}",
            e
        )));

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.create_project(req_data).await;

    respond!(json, result)
}

async fn delete_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, |e| AppError::Serialization(
        format!("Invalid JSON Body: {}", e)
    ));

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.delete_project(target).await;

    match result {
        Ok(true) => Response::empty().map(|r| r.with_status(204)),
        Ok(false) => Response::error("Not Found", 404),
        Err(e) => Ok(map_error_to_response(e)),
    }
}

async fn pop_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let req_data: PopProjectRequest =
        unwrap_or_resp!(req.json().await, |e| AppError::Serialization(format!(
            "Invalid JSON Body: {}",
            e
        )));
    let target = DeleteTarget { id: req_data.id };

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.pop_project(target).await;

    respond!(json, result)
}

async fn switch_monitor(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let cmd: SwitchMonitorRequest = unwrap_or_resp!(req.json().await, |e| AppError::Serialization(
        format!("Invalid JSON Body: {}", e)
    ));

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.switch_monitor(cmd.unique_key, cmd.paused).await;

    respond!(json, result)
}

async fn trigger_check(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let cmd: TriggerCheckRequest = unwrap_or_resp!(req.json().await, |e| AppError::Serialization(
        format!("Invalid JSON Body: {}", e)
    ));

    let registry = unwrap_or_resp!(
        DoProjectRegistry::new(&ctx.env, &cfg.registry_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&registry);
    let result = logic.trigger_check(cmd.unique_key).await;

    respond!(json, result)
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

    let router = Router::new();
    router
        .get_async("/api/projects", list_projects)
        .post_async("/api/projects", create_project)
        .delete_async("/api/projects", delete_project)
        .delete_async("/api/projects/pop", pop_project)
        .post_async("/api/projects/switch", switch_monitor)
        .post_async("/api/projects/trigger", trigger_check)
        .options_async("/api/projects", |_, _| async { Response::empty() })
        .options_async("/api/projects/pop", |_, _| async { Response::empty() })
        .options_async("/api/projects/switch", |_, _| async { Response::empty() })
        .options_async("/api/projects/trigger", |_, _| async { Response::empty() })
        .run(req, env)
        .await?
        .with_cors(&cors)
}
