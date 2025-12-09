use worker::*;

pub mod error;
pub mod logic; // 引入 Logic 模块
mod repository;
pub mod service; // 引入 Error 模块

pub(crate) mod utils {
    pub mod request;
}

use error::AppError;
use logic::AdminLogic;
use repository::DoProjectRepository;
use service::{EnvSecretResolver, WatchdogService};
use utils::request::WorkerHttpClient;
use verwatch_shared::{CreateProjectRequest, DeleteTarget, HEADER_AUTH_KEY, ProjectConfig};

// =========================================================
// 常量定义
// =========================================================
const DEFAULT_DO_BINDING: &str = "PROJECT_STORE";
const DEFAULT_SECRET_VAR_NAME: &str = "ADMIN_SECRET";
const DEFAULT_GITHUB_TOKEN_VAR_NAME: &str = "GITHUB_TOKEN";
const DEFAULT_PAT_VAR_NAME: &str = "MY_GITHUB_PAT";

// =========================================================
// 宏定义 (包含日志和响应处理)
// =========================================================

#[cfg(target_arch = "wasm32")]
macro_rules! log_error { ($($t:tt)*) => (worker::console_error!($($t)*)) }
#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_error { ($($t:tt)*) => (eprintln!($($t)*)) }

#[cfg(target_arch = "wasm32")]
macro_rules! log_info { ($($t:tt)*) => (worker::console_log!($($t)*)) }
#[cfg(not(target_arch = "wasm32"))]
macro_rules! log_info { ($($t:tt)*) => (println!($($t)*)) }

// 辅助函数：将 AppError 映射为 Worker Response
// 现在的映射逻辑利用了 AppError::status_code()，更加简洁且内聚
fn map_error_to_response(e: AppError) -> worker::Response {
    let status = e.status_code();
    let msg = e.to_string();

    // 对于 5xx 错误，记录日志以便排查
    if status >= 500 {
        log_error!("Internal Error [{}]: {}", e.error_code(), msg);
        // 在生产环境，通常建议隐藏 500 错误的具体细节，返回通用消息
        // 这里为了演示方便，返回了标准错误文本
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
    (empty, $expr:expr) => {
        match $expr {
            Ok(_) => Response::empty(),
            Err(e) => Ok(map_error_to_response(e)),
        }
    };
}

// 辅助宏：处理 Option/Result 类型的 unwrapping
macro_rules! unwrap_or_resp {
    ($expr:expr, $err_mapper:expr) => {
        match $expr {
            Ok(v) => v,
            // $err_mapper 可以是一个变体构造器 (AppError::Store) 或闭包
            Err(e) => return Ok(map_error_to_response($err_mapper(e.to_string()))),
        }
    };
}

// =========================================================
// 运行时配置与鉴权
// =========================================================

struct RuntimeConfig {
    do_binding: String,
    admin_secret_name: String,
    github_token_name: String,
    pat_token_name: String,
}

impl RuntimeConfig {
    fn new(env: &Env) -> Self {
        Self {
            do_binding: env
                .var("DO_BINDING")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_DO_BINDING.to_string()),
            admin_secret_name: env
                .var("ADMIN_SECRET_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_SECRET_VAR_NAME.to_string()),
            github_token_name: env
                .var("GITHUB_TOKEN_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_GITHUB_TOKEN_VAR_NAME.to_string()),
            pat_token_name: env
                .var("PAT_TOKEN_NAME")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| DEFAULT_PAT_VAR_NAME.to_string()),
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

    let repo = unwrap_or_resp!(
        DoProjectRepository::new(&ctx.env, &cfg.do_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&repo);
    let result = logic.list_projects().await;

    respond!(json, result)
}

async fn create_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    // 使用 AppError::Serialization 专门处理 JSON 解析错误
    let req_data: CreateProjectRequest =
        unwrap_or_resp!(req.json().await, |e| AppError::Serialization(format!(
            "Invalid JSON Body: {}",
            e
        )));

    let repo = unwrap_or_resp!(
        DoProjectRepository::new(&ctx.env, &cfg.do_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&repo);
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
    let repo = unwrap_or_resp!(
        DoProjectRepository::new(&ctx.env, &cfg.do_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&repo);
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

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, |e| AppError::Serialization(
        format!("Invalid JSON Body: {}", e)
    ));
    let repo = unwrap_or_resp!(
        DoProjectRepository::new(&ctx.env, &cfg.do_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&repo);
    let result = logic.pop_project(target).await;

    respond!(json, result)
}

async fn toggle_pause_project(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let cfg = RuntimeConfig::new(&ctx.env);
    if let Err(e) = ensure_admin_auth(&req, &ctx.env, &cfg) {
        return Ok(map_error_to_response(e));
    }

    let target: DeleteTarget = unwrap_or_resp!(req.json().await, |e| AppError::Serialization(
        format!("Invalid JSON Body: {}", e)
    ));
    let repo = unwrap_or_resp!(
        DoProjectRepository::new(&ctx.env, &cfg.do_binding),
        AppError::Store
    );

    let logic = AdminLogic::new(&repo);
    let result = logic.toggle_pause(target).await;

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
            Method::Patch,
        ])
        .with_allowed_headers(vec!["Content-Type", HEADER_AUTH_KEY]);

    let router = Router::new();
    router
        .get_async("/api/projects", list_projects)
        .post_async("/api/projects", create_project)
        .delete_async("/api/projects", delete_project)
        .delete_async("/api/projects/pop", pop_project)
        .post_async("/api/projects/toggle_pause", toggle_pause_project)
        .options_async("/api/projects", |_, _| async { Response::empty() })
        .options_async("/api/projects/pop", |_, _| async { Response::empty() })
        .options_async("/api/projects/toggle_pause", |_, _| async {
            Response::empty()
        })
        .run(req, env)
        .await?
        .with_cors(&cors)
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    let config = RuntimeConfig::new(&env);
    let client = WorkerHttpClient;
    let secret_resolver = EnvSecretResolver(&env);

    // Repository 初始化失败是严重的运行时错误，直接 return
    let repo = match DoProjectRepository::new(&env, &config.do_binding) {
        Ok(r) => r,
        Err(e) => {
            log_error!("Repo Init Error: {}", e);
            return;
        }
    };

    let global_read = env
        .secret(&config.github_token_name)
        .ok()
        .map(|s| s.to_string());
    let global_dispatch = env
        .secret(&config.pat_token_name)
        .map(|s| s.to_string())
        .unwrap_or_default();

    let service = WatchdogService::new(
        repo,
        &client,
        &secret_resolver,
        global_read,
        global_dispatch,
    );

    // run_all 内部已经处理了 Result -> String 的转换
    match service.run_all().await {
        Ok(msg) => log_info!("Cron Success: {}", msg),
        Err(e) => log_error!("Cron Error: {}", e), // 这里的 e 是 AppError，会自动 display
    }
}
