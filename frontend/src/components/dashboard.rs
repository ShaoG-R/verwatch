use crate::api::VerWatchApi;
use crate::auth::{logout, use_auth};
use crate::components::add_project_dialog::AddProjectDialog;
use crate::components::icons::*;
use silex::prelude::*;
use verwatch_shared::{CreateProjectRequest, Date, MonitorState, ProjectConfig};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

// JS 格式化函数绑定 (定义在 index.html)
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = formatCountdown)]
    fn format_countdown(secs: f64) -> String;
}

// --- Logic Layer: Dashboard Store ---

#[derive(Clone)]
pub struct DashboardStore {
    pub projects: ReadSignal<Vec<ProjectConfig>>,
    pub loading: ReadSignal<bool>,
    pub tick: ReadSignal<u64>,
    pub notification: ReadSignal<Option<(String, bool)>>,
    // Actions
    pub refresh: Callback<()>,
    pub add_project: Callback<CreateProjectRequest>,
    pub delete_project: Callback<String>,
    pub switch_monitor: Callback<(String, bool)>,
    pub trigger_check: Callback<String>,
}

// --- API Action Runner: 消除重复的 API 调用逻辑 ---

#[derive(Clone, Copy)]
struct ApiActionRunner {
    auth_state: ReadSignal<crate::auth::AuthState>,
    set_notification: WriteSignal<Option<(String, bool)>>,
    load_projects: Callback<()>,
}

impl ApiActionRunner {
    /// 执行 API 操作，成功后刷新列表并显示通知
    fn run<T, F, Fut>(
        self,
        api_call: F,
        on_success: impl FnOnce(T) -> String + 'static,
        error_prefix: &'static str,
    ) where
        F: FnOnce(VerWatchApi) -> Fut + 'static,
        Fut: std::future::Future<Output = Result<T, String>> + 'static,
        T: 'static,
    {
        if let Some(api) = self.auth_state.get().api.clone() {
            let set_notification = self.set_notification;
            let load_projects = self.load_projects;
            spawn_local(async move {
                match api_call(api).await {
                    Ok(result) => {
                        set_notification.set(Some((on_success(result), false)));
                        load_projects.call(());
                    }
                    Err(e) => {
                        set_notification.set(Some((format!("{}: {}", error_prefix, e), true)))
                    }
                }
            });
        }
    }
}

pub fn use_dashboard_store() -> DashboardStore {
    expect_context::<DashboardStore>()
}

pub fn use_provide_dashboard_store() -> DashboardStore {
    let (projects, set_projects) = signal(Vec::<ProjectConfig>::new());
    let (loading, set_loading) = signal(true);
    let (notification, set_notification) = signal(Option::<(String, bool)>::None);
    let (tick, set_tick) = signal(0u64);

    let auth = use_auth();
    let auth_state = auth.state;

    // --- Action Implementations ---

    let load_projects = Callback::new(move |_| {
        let state = auth_state.get();
        if let Some(api) = state.api.as_ref() {
            let api = api.clone();
            set_loading.set(true);
            spawn_local(async move {
                match api.get_projects().await {
                    Ok(data) => set_projects.set(data),
                    Err(e) => set_notification.set(Some((format!("加载项目失败: {}", e), true))),
                }
                set_loading.set(false);
            });
        }
    });

    // 创建 runner 实例，封装共享依赖
    let runner = ApiActionRunner {
        auth_state,
        set_notification,
        load_projects,
    };

    let add_project = Callback::new(move |req| {
        runner.run(
            |api| async move { api.add_project(req).await },
            |_| "监控添加成功".to_string(),
            "添加监控失败",
        );
    });

    let delete_project = Callback::new(move |id: String| {
        runner.run(
            |api| async move { api.delete_project(id).await },
            |deleted| {
                if deleted {
                    "监控已删除"
                } else {
                    "监控不存在 (已清理)"
                }
                .to_string()
            },
            "删除监控失败",
        );
    });

    let switch_monitor = Callback::new(move |(id, paused): (String, bool)| {
        runner.run(
            move |api| async move { api.switch_monitor(id, paused).await },
            |new_state| {
                if new_state {
                    "监控已暂停"
                } else {
                    "监控已恢复"
                }
                .to_string()
            },
            "切换状态失败",
        );
    });

    let trigger_check = Callback::new(move |id: String| {
        runner.run(
            |api| async move { api.trigger_check(id).await },
            |_| "检查已触发".to_string(),
            "触发失败",
        );
    });

    // --- Timer & Auto Refresh Logic ---
    Effect::new(move |_| {
        if !auth_state.get().is_authenticated {
            return;
        }

        // Start 1s Tick using auto-cleanup use_interval
        let _ = use_interval(std::time::Duration::from_secs(1), move || {
            set_tick.update(|t| *t = t.wrapping_add(1));
        });

        // Auto refresh check
        Effect::new(move |_| {
            let _ = tick.get();
            let list = projects.get();
            let now = Date::now_timestamp();

            // Allow refresh if any project is expired
            let needs_refresh = list.iter().any(|p| {
                matches!(&p.state, MonitorState::Running { next_check_at } if *next_check_at <= now)
            });

            // Prevent concurrent refreshes
            if needs_refresh && !loading.get_untracked() {
                load_projects.call(());
            }
        });

        // Auto-clear notification
        Effect::new(move |_| {
            if notification.get().is_some() {
                let _ = use_timeout(std::time::Duration::from_secs(3), move || {
                    set_notification.set(None)
                });
            }
        });
    });

    // Initial Load when authenticated
    Effect::new(move |_| {
        let state = auth_state.get();
        if state.is_authenticated && !state.is_loading {
            load_projects.call(());
        }
    });

    let store = DashboardStore {
        projects,
        loading,
        tick,
        notification,
        refresh: load_projects,
        add_project,
        delete_project,
        switch_monitor,
        trigger_check,
    };

    provide_context(store.clone());
    store
}

// --- UI Layer: Components ---

#[component]
pub fn DashboardPage() -> impl View {
    // 1. Initialize Store (Provides Context)
    let store = use_provide_dashboard_store();
    let auth = use_auth();
    let auth_state = auth.state;

    // 注意：不再需要手动重定向逻辑
    // 路由服务会监听认证状态变化并自动处理重定向

    let backend_url = Signal::derive(move || auth_state.get().backend_url);

    div![
        NotificationToast().notification(store.notification),
        DashboardNavbar()
            .backend_url(backend_url)
            .on_logout(Callback::new(move |_: web_sys::MouseEvent| {
                logout(&auth);
            })),
        DashboardStats(),
        ProjectsTable()
    ]
    .class("max-w-7xl mx-auto w-full flex-1 flex flex-col gap-8 min-h-0")
    .style("display: flex; flex-direction: column;")
}

#[component]
fn NotificationToast(notification: ReadSignal<Option<(String, bool)>>) -> impl View {
    Show::new(notification.map(|n| n.is_some()), move || {
        div(
            div(span(notification.map(|n| n.clone().unwrap().0))).class(notification.map(|n| {
                let (_, is_err) = n.clone().unwrap();
                if is_err {
                    "alert alert-error shadow-lg"
                } else {
                    "alert alert-success shadow-lg"
                }
            })),
        )
        .class("toast toast-top toast-end z-50")
    })
}

#[component]
fn DashboardNavbar(
    backend_url: Signal<String>,
    #[prop(into)] on_logout: Callback<web_sys::MouseEvent>,
) -> impl View {
    let store = use_dashboard_store();

    div![
        div![
            Radio()
                .style("height: 24px; width: 24px;")
                .class("text-primary animate-pulse"),
            a("VerWatch 控制面板").class("btn btn-ghost text-xl"),
            span(("已连接至 ", backend_url)).class("badge badge-neutral hidden md:inline-flex")
        ]
        .class("flex-1 gap-2"),
        div![
            AddProjectDialog().on_add(Callback::new(move |req| store.add_project.call(req))),
            button((LogOut().style("height: 16px; width: 16px;"), " 断开连接"))
                .on(event::click, move |e| on_logout.call(e))
                .class("btn btn-outline btn-error gap-2")
        ]
        .class("flex-none gap-2")
    ]
    .class("navbar bg-base-100 rounded-box shadow-xl")
}

#[component]
fn DashboardStats() -> impl View {
    let store = use_dashboard_store();
    let total_monitors = move || store.projects.with(|p| p.len());

    div![
        div![
            div(svg(path()
                .attr("stroke-linecap", "round")
                .attr("stroke-linejoin", "round")
                .attr("stroke-width", "2")
                .attr(
                    "d",
                    "M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                ))
            .attr("xmlns", "http://www.w3.org/2000/svg")
            .attr("fill", "none")
            .attr("viewBox", "0 0 24 24")
            .class("inline-block w-8 h-8 stroke-current"))
            .class("stat-figure text-primary"),
            div("监控总数").class("stat-title"),
            div(total_monitors).class("stat-value text-primary")
        ]
        .class("stat"),
        div![
            div(svg(path()
                .attr("stroke-linecap", "round")
                .attr("stroke-linejoin", "round")
                .attr("stroke-width", "2")
                .attr("d", "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"))
            .attr("xmlns", "http://www.w3.org/2000/svg")
            .attr("fill", "none")
            .attr("viewBox", "0 0 24 24")
            .class("inline-block w-8 h-8 stroke-current"))
            .class("stat-figure text-success"),
            div("系统状态").class("stat-title"),
            div("运行中").class("stat-value text-success")
        ]
        .class("stat"),
        div![
            div("更新策略").class("stat-title"),
            div("自动 (定时)").class("stat-value text-secondary text-2xl"),
            div("Workers 自动调度").class("stat-desc")
        ]
        .class("stat")
    ]
    .class("stats shadow w-full stats-vertical md:stats-horizontal bg-base-100")
}

#[component]
fn ProjectsTable() -> impl View {
    let store = use_dashboard_store();
    let total_monitors = move || store.projects.with(|p| p.len());

    div(div![
        div![
            div![
                h3("活跃监控").class("card-title"),
                p((
                    "管理您的仓库监控列表。目前共有 ",
                    total_monitors,
                    " 个监控项。"
                ))
                .class("text-base-content/70 text-sm")
            ],
            button(RefreshCw().style(store.loading.map(|l| {
                if l {
                    "height: 20px; width: 20px; animation: spin 1s linear infinite;"
                } else {
                    "height: 20px; width: 20px;"
                }
            })))
            .on(event::click, move |_| store.refresh.call(()))
            .disabled(store.loading)
            .class("btn btn-ghost btn-circle")
        ]
        .class("flex items-center justify-between p-6 pb-2 flex-none"),
        div(table![
            thead(tr((
                th("上游"),
                th("目标"),
                th("触发模式").class("hidden md:table-cell"),
                th("下次检查").class("hidden md:table-cell"),
                th("密钥").class("hidden lg:table-cell"),
                th(())
            ))),
            tbody((
                // 空状态
                Show::new(
                    move || total_monitors() == 0 && !store.loading.get(),
                    || tr(td("未配置监控。添加一个以开始。")
                        .attr("colspan", "5")
                        .class("text-center py-8 text-base-content/50"))
                ),
                // 加载状态
                Show::new(
                    move || store.loading.get() && total_monitors() == 0,
                    || tr(td((
                        span(()).class("loading loading-spinner loading-md"),
                        " 加载中..."
                    ))
                    .attr("colspan", "5")
                    .class("text-center py-8 text-base-content/50"))
                ),
                // 项目列表
                For::new(
                    store.projects,
                    |p| {
                        match &p.state {
                            MonitorState::Paused => format!("{}|paused", p.unique_key),
                            MonitorState::Running { next_check_at } => {
                                format!(
                                    "{}|running|{}",
                                    p.unique_key,
                                    next_check_at.as_millis_i64()
                                )
                            }
                        }
                    },
                    move |project| ProjectRow().project(project)
                )
            ))
        ]
        .class("table table-zebra w-full"))
        .class("overflow-x-auto w-full")
    ]
    .class("card-body p-0 flex flex-col"))
    .class("card bg-base-100 shadow-xl min-h-0")
}

struct ProjectRowDisplay {
    upstream: String,
    target: String,
    mode: String,
    secret: String,
}

impl From<&ProjectConfig> for ProjectRowDisplay {
    fn from(p: &ProjectConfig) -> Self {
        Self {
            upstream: format!(
                "{} / {}",
                p.request.base_config.upstream_owner, p.request.base_config.upstream_repo
            ),
            target: format!(
                "{} / {}",
                p.request.base_config.my_owner, p.request.base_config.my_repo
            ),
            mode: format!("{:?}", p.request.comparison_mode),
            secret: p
                .request
                .dispatch_token_secret
                .clone()
                .unwrap_or("全局".to_string()),
        }
    }
}

#[component]
fn ProjectRow(project: ProjectConfig) -> impl View {
    let store = use_dashboard_store();
    let id = project.unique_key.clone();
    let is_paused = project.state.is_paused();
    let state_for_countdown = project.state.clone();
    let state_for_badge = project.state.clone();
    let display = ProjectRowDisplay::from(&project);

    // Countdown Text - 调用 JS 格式化函数
    let countdown_text = move || {
        let _ = store.tick.get(); // Subscribe to tick
        match &state_for_countdown {
            MonitorState::Paused => "--".to_string(),
            MonitorState::Running { next_check_at } => {
                let now = Date::now_timestamp();
                let secs = (*next_check_at - now).as_secs() as f64;
                format_countdown(secs)
            }
        }
    };

    let (id_pause, id_check, id_del) = (id.clone(), id.clone(), id.clone());

    tr![
        td(div![
            Github().style("height: 16px; width: 16px; opacity: 0.5;"),
            display.upstream,
            Show::new(
                move || is_paused,
                || {
                    span((Pause().style("height: 12px; width: 12px;"), " 已暂停"))
                        .class("badge badge-warning badge-sm gap-1")
                }
            )
        ]
        .class("flex items-center gap-2 font-mono text-sm font-bold")),
        td(div![
            GitFork().style("height: 16px; width: 16px; opacity: 0.5;"),
            display.target
        ]
        .class("flex items-center gap-2 font-mono text-sm opacity-70")),
        td(div(display.mode).class("badge badge-accent badge-outline"))
            .class("hidden md:table-cell"),
        td(div![
            Clock().style("height: 12px; width: 12px; margin-right: 4px;"),
            countdown_text
        ]
        .class(move || {
            let _ = store.tick.get();
            let base = "badge badge-sm font-mono";
            match &state_for_badge {
                MonitorState::Paused => format!("{} badge-ghost", base),
                MonitorState::Running { next_check_at } => {
                    let now = Date::now_timestamp();
                    let secs = (*next_check_at - now).as_secs() as i64;
                    if secs <= 60 {
                        format!("{} badge-error animate-pulse", base)
                    } else if secs <= 300 {
                        format!("{} badge-warning", base)
                    } else {
                        format!("{} badge-info", base)
                    }
                }
            }
        }))
        .class("hidden md:table-cell"),
        td(display.secret).class("hidden lg:table-cell font-mono text-xs opacity-50"),
        td(div![
            div(MoreHorizontal().style("height: 16px; width: 16px;"))
                .attr("tabindex", "0")
                .attr("role", "button")
                .class("btn btn-ghost btn-sm btn-square"),
            ul![
                li(a(Dynamic::bind(
                    move || is_paused,
                    |paused| {
                        if paused {
                            div![
                                Play().style("height: 16px; width: 16px; margin-right: 8px;"),
                                "恢复监控"
                            ]
                            .into_any()
                        } else {
                            div![
                                Pause().style("height: 16px; width: 16px; margin-right: 8px;"),
                                "暂停监控"
                            ]
                            .into_any()
                        }
                    }
                ))
                .on(event::click, move |_| store
                    .switch_monitor
                    .call((id_pause.clone(), !is_paused)))),
                li(a((
                    RefreshCw().style("height: 16px; width: 16px; margin-right: 8px;"),
                    "立即触发检查"
                ))
                .on(event::click, move |_| store
                    .trigger_check
                    .call(id_check.clone()))),
                li(a((
                    Trash2().style("height: 16px; width: 16px; margin-right: 8px;"),
                    "删除"
                ))
                .class("text-error hover:bg-error/10")
                .on(event::click, move |_| store
                    .delete_project
                    .call(id_del.clone())))
            ]
            .attr("tabindex", "0")
            .class("dropdown-content z-[1] menu p-2 shadow bg-base-200 rounded-box w-52")
        ]
        .class("dropdown dropdown-end"))
    ]
    .classes(classes![
        "opacity-50" => move || is_paused,
        "grayscale" => move || is_paused,
        "bg-base-200" => move || is_paused
    ])
}
