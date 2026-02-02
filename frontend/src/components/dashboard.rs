use crate::api::VerWatchApi;
use crate::auth::{logout, use_auth};
use crate::components::add_project_dialog::AddProjectDialog;
use crate::components::icons::*;
use silex::prelude::*;
use verwatch_shared::{CreateProjectRequest, Date, MonitorState, ProjectConfig};
use wasm_bindgen::prelude::*;

// JS 格式化函数绑定 (定义在 index.html)
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = formatCountdown)]
    fn format_countdown(secs: f64) -> String;
}

// --- Logic Layer: Dashboard Store ---

#[derive(Clone)]
pub struct DashboardStore {
    pub resource: Resource<Vec<ProjectConfig>, String>,
    pub tick: ReadSignal<u64>,
    pub notification: ReadSignal<Option<(String, bool)>>,
    // Actions
    pub refresh: Callback<()>,
    pub add_project: Mutation<CreateProjectRequest, ProjectConfig, String>,
    pub delete_project: Mutation<String, (String, bool), String>,
    pub switch_monitor: Mutation<(String, bool), (bool, bool), String>,
    pub trigger_check: Mutation<String, (), String>,
}

pub fn use_dashboard_store() -> DashboardStore {
    expect_context::<DashboardStore>()
}

pub fn use_provide_dashboard_store() -> DashboardStore {
    let (notification, set_notification) = signal(Option::<(String, bool)>::None);
    let (tick, set_tick) = signal(0u64);

    let auth = use_auth();
    let auth_state = auth.state;

    // Helper for notifications
    let notify = move |msg: String, is_err: bool| {
        set_notification.set(Some((msg, is_err)));
    };

    // --- Resource Implementation ---
    let resource: Resource<Vec<ProjectConfig>, String> = Resource::new(
        move || auth_state.get().api,
        move |api_opt: Option<VerWatchApi>| async move {
            if let Some(api) = api_opt {
                api.get_projects().await.map_err(|e| e.to_string())
            } else {
                Ok(Vec::new())
            }
        },
    );

    // Error handling for resource
    Effect::new(move |_| {
        if let ResourceState::Error(err) = resource.state.get() {
            notify(format!("加载项目失败: {}", err), true);
        }
    });

    let refresh = Callback::new(move |_| resource.refetch());

    // --- Mutations ---

    // 1. Add Project
    let add_project = Mutation::new(move |req: CreateProjectRequest| {
        let api = auth_state.get().api;
        async move {
            let api = api.ok_or("未连接到服务器".to_string())?;
            api.add_project(req).await
        }
    });

    Effect::new(move |_| {
        if let MutationState::Success(new_project) = add_project.state.get() {
            notify("监控添加成功".to_string(), false);
            // Optimization: Local update
            resource.update(|list| list.push(new_project.clone()));
        } else if let MutationState::Error(err) = add_project.state.get() {
            notify(format!("添加监控失败: {}", err), true);
        }
    });

    // 2. Delete Project
    let delete_project = Mutation::new(move |id: String| {
        let api = auth_state.get().api;
        async move {
            let api = api.ok_or("未连接到服务器".to_string())?;
            // Return (id, result) to use ID in success callback
            api.delete_project(id.clone()).await.map(|res| (id, res))
        }
    });

    Effect::new(move |_| {
        if let MutationState::Success((id, deleted)) = delete_project.state.get() {
            if deleted {
                notify("监控已删除".to_string(), false);
                resource.update(|list| {
                    if let Some(pos) = list.iter().position(|p| &p.unique_key == &id) {
                        list.remove(pos);
                    }
                });
            } else {
                notify("监控不存在 (已清理)".to_string(), false);
                resource.refetch();
            }
        } else if let MutationState::Error(err) = delete_project.state.get() {
            notify(format!("删除监控失败: {}", err), true);
        }
    });

    // 3. Switch Monitor
    let switch_monitor = Mutation::new(move |(id, paused): (String, bool)| {
        let api = auth_state.get().api;
        async move {
            let api = api.ok_or("未连接到服务器".to_string())?;
            api.switch_monitor(id, paused)
                .await
                .map(|res| (paused, res))
        }
    });

    Effect::new(move |_| {
        if let MutationState::Success((paused, _)) = switch_monitor.state.get() {
            let msg = if paused {
                "监控已暂停"
            } else {
                "监控已恢复"
            };
            notify(msg.to_string(), false);
            resource.refetch();
        } else if let MutationState::Error(err) = switch_monitor.state.get() {
            notify(format!("切换状态失败: {}", err), true);
        }
    });

    // 4. Trigger Check
    let trigger_check = Mutation::new(move |id: String| {
        let api = auth_state.get().api;
        async move {
            let api = api.ok_or("未连接到服务器".to_string())?;
            api.trigger_check(id).await
        }
    });

    Effect::new(move |_| {
        if let MutationState::Success(_) = trigger_check.state.get() {
            notify("检查已触发".to_string(), false);
            resource.refetch();
        } else if let MutationState::Error(err) = trigger_check.state.get() {
            notify(format!("触发失败: {}", err), true);
        }
    });

    // --- Auto Helpers ---
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
            let state = resource.state.get();

            // 注意：我们只在 Resource 加载成功且非 loading 时检查
            if matches!(state, ResourceState::Loading | ResourceState::Reloading(_)) {
                return;
            }

            if let Some(list) = resource.get_data() {
                let now = Date::now_timestamp();

                // Allow refresh if any project is expired
                let needs_refresh = list.iter().any(|p| {
                    matches!(&p.state, MonitorState::Running { next_check_at } if next_check_at <= &now)
                });

                // Prevent concurrent refreshes is handled by Resource internal state mostly,
                // but explicit check helps avoiding spam
                if needs_refresh {
                    resource.refetch();
                }
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

    let store = DashboardStore {
        resource,
        tick,
        notification,
        refresh,
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
            AddProjectDialog().on_add(Callback::new(move |req| store.add_project.mutate(req))),
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
    let total_monitors = move || {
        store
            .resource
            .get_data()
            .map(|v| v.len())
            .unwrap_or_default()
    };

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
    let project_list = move || store.resource.get_data().unwrap_or_default();
    let total_monitors = move || project_list().len();
    let is_loading = Memo::new(move |_| {
        matches!(
            store.resource.state.get(),
            ResourceState::Loading | ResourceState::Reloading(_)
        )
    });

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
            button(RefreshCw().style(is_loading.map(|l| {
                if l {
                    "height: 20px; width: 20px; animation: spin 1s linear infinite;"
                } else {
                    "height: 20px; width: 20px;"
                }
            })))
            .on(event::click, move |_| store.refresh.call(()))
            .disabled(is_loading)
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
                    move || total_monitors() == 0 && !is_loading.get(),
                    || tr(td("未配置监控。添加一个以开始。")
                        .attr("colspan", "5")
                        .class("text-center py-8 text-base-content/50"))
                ),
                // 加载状态
                Show::new(
                    move || is_loading.get() && total_monitors() == 0,
                    || tr(td((
                        span(()).class("loading loading-spinner loading-md"),
                        " 加载中..."
                    ))
                    .attr("colspan", "5")
                    .class("text-center py-8 text-base-content/50"))
                ),
                // 项目列表
                For::new(
                    project_list,
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
                    .mutate((id_pause.clone(), !is_paused)))),
                li(a((
                    RefreshCw().style("height: 16px; width: 16px; margin-right: 8px;"),
                    "立即触发检查"
                ))
                .on(event::click, move |_| store
                    .trigger_check
                    .mutate(id_check.clone()))),
                li(a((
                    Trash2().style("height: 16px; width: 16px; margin-right: 8px;"),
                    "删除"
                ))
                .class("text-error hover:bg-error/10")
                .on(event::click, move |_| store
                    .delete_project
                    .mutate(id_del.clone())))
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
