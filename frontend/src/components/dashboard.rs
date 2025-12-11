use crate::api::VerWatchApi;
use crate::auth::{AuthContext, logout, use_auth};
use crate::components::add_project_dialog::AddProjectDialog;
use crate::components::icons::*;
use crate::web::{Interval, use_navigate};
use leptos::prelude::*;
use leptos::task::spawn_local;
use verwatch_shared::{CreateProjectRequest, Date, MonitorState, ProjectConfig};
use wasm_bindgen::prelude::*;

// JS 格式化函数绑定 (定义在 index.html)
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = formatCountdown)]
    fn format_countdown(secs: i64) -> String;
}

// --- Logic Layer: Dashboard Store ---

#[derive(Clone)]
pub struct DashboardStore {
    pub projects: Signal<Vec<ProjectConfig>>,
    pub loading: Signal<bool>,
    pub tick: Signal<u64>,
    pub notification: Signal<Option<(String, bool)>>,
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
                        load_projects.run(());
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
    use_context::<DashboardStore>().expect("DashboardStore must be used within a DashboardProvider")
}

pub fn use_provide_dashboard_store() -> DashboardStore {
    let (projects, set_projects) = signal(Vec::<ProjectConfig>::new());
    let (loading, set_loading) = signal(true);
    let (notification, set_notification) = signal(Option::<(String, bool)>::None);
    let (tick, set_tick) = signal(0u64);

    let AuthContext(auth_state, _) = use_auth();

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

        // Start 1s Tick
        let handle = Interval::new(1000, move || {
            set_tick.update(|t| *t = t.wrapping_add(1));
        });

        let interval_handle = StoredValue::new_local(handle);

        let cleanup_tick = tick.clone();

        // Auto refresh check
        Effect::new(move |_| {
            let _ = cleanup_tick.get();
            let list = projects.get();
            let now = Date::now_timestamp();

            // Allow refresh if any project is expired
            let needs_refresh = list.iter().any(|p| {
                matches!(&p.state, MonitorState::Running { next_check_at } if *next_check_at <= now)
            });

            // Prevent concurrent refreshes
            if needs_refresh && !loading.get_untracked() {
                load_projects.run(());
            }
        });

        // Auto-clear notification
        Effect::new(move |_| {
            if notification.get().is_some() {
                set_timeout(
                    move || set_notification.set(None),
                    std::time::Duration::from_secs(3),
                );
            }
        });

        on_cleanup(move || {
            interval_handle.dispose();
        });
    });

    // Initial Load when authenticated
    Effect::new(move |_| {
        let state = auth_state.get();
        if state.is_authenticated && !state.is_loading {
            // Only load if empty? Or always refresh? Let's just always load on component mount/auth
            load_projects.run(());
        }
    });

    let store = DashboardStore {
        projects: projects.into(),
        loading: loading.into(),
        tick: tick.into(),
        notification: notification.into(),
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
pub fn DashboardPage() -> impl IntoView {
    // 1. Initialize Store (Provides Context)
    let store = use_provide_dashboard_store();
    let navigate = use_navigate();
    let AuthContext(auth_state, set_auth) = use_auth();

    // Redirect if not authenticated
    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            let state = auth_state.get();
            if !state.is_loading && !state.is_authenticated {
                navigate("/");
            }
        }
    });

    let backend_url = Signal::derive(move || auth_state.get().backend_url);

    view! {
        <div class="h-screen bg-base-200 p-4 md:p-8 font-sans flex flex-col overflow-hidden">
            <div class="max-w-7xl mx-auto w-full flex-1 flex flex-col gap-8 min-h-0">
                <NotificationToast notification=store.notification.into() />

                <DashboardNavbar
                    backend_url=backend_url
                    on_logout=Callback::new(move |_| { logout(set_auth); navigate("/"); })
                />

                <DashboardStats />

                <ProjectsTable />
            </div>
        </div>
    }
}

#[component]
fn NotificationToast(notification: Signal<Option<(String, bool)>>) -> impl IntoView {
    view! {
        <Show when=move || notification.get().is_some()>
            <div class="toast toast-top toast-end z-50">
                <div class=move || {
                    let (_, is_err) = notification.get().unwrap();
                    if is_err { "alert alert-error shadow-lg" } else { "alert alert-success shadow-lg" }
                }>
                    <span>{move || notification.get().unwrap().0}</span>
                </div>
            </div>
        </Show>
    }
}

#[component]
fn DashboardNavbar(
    backend_url: Signal<String>,
    on_logout: Callback<leptos::ev::MouseEvent>,
) -> impl IntoView {
    let store = use_dashboard_store();

    view! {
        <div class="navbar bg-base-100 rounded-box shadow-xl">
            <div class="flex-1 gap-2">
                <Radio attr:class="text-primary h-6 w-6 animate-pulse" />
                <a class="btn btn-ghost text-xl">"VerWatch 控制面板"</a>
                <span class="badge badge-neutral hidden md:inline-flex">
                    "已连接至 " {backend_url}
                </span>
            </div>
            <div class="flex-none gap-2">
                <AddProjectDialog on_add=move |req| store.add_project.run(req) />
                <button on:click=move |e| on_logout.run(e) class="btn btn-outline btn-error gap-2">
                    <LogOut attr:class="h-4 w-4" /> "断开连接"
                </button>
            </div>
        </div>
    }
}

#[component]
fn DashboardStats() -> impl IntoView {
    let store = use_dashboard_store();
    let total_monitors = move || store.projects.with(|p| p.len());

    view! {
        <div class="stats shadow w-full stats-vertical md:stats-horizontal bg-base-100">
            <div class="stat">
                <div class="stat-figure text-primary">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="inline-block w-8 h-8 stroke-current"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                </div>
                <div class="stat-title">"监控总数"</div>
                <div class="stat-value text-primary">{total_monitors}</div>
            </div>

            <div class="stat">
                <div class="stat-figure text-success">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="inline-block w-8 h-8 stroke-current"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                </div>
                <div class="stat-title">"系统状态"</div>
                <div class="stat-value text-success">"运行中"</div>
            </div>

            <div class="stat">
                    <div class="stat-title">"更新策略"</div>
                    <div class="stat-value text-secondary text-2xl">"自动 (定时)"</div>
                    <div class="stat-desc">"Workers 自动调度"</div>
            </div>
        </div>
    }
}

#[component]
fn ProjectsTable() -> impl IntoView {
    let store = use_dashboard_store();

    let total_monitors = move || store.projects.with(|p| p.len());

    view! {
        <div class="card bg-base-100 shadow-xl flex-1 flex flex-col min-h-0">
            <div class="card-body p-0 flex flex-col h-full overflow-hidden">
                <div class="flex items-center justify-between p-6 pb-2 flex-none">
                    <div>
                        <h3 class="card-title">"活跃监控"</h3>
                        <p class="text-base-content/70 text-sm">"管理您的仓库监控列表。目前共有 " {total_monitors} " 个监控项。"</p>
                    </div>
                    <button on:click=move |_| store.refresh.run(()) disabled=move || store.loading.get() class="btn btn-ghost btn-circle">
                        <RefreshCw attr:class=move || if store.loading.get() { "h-5 w-5 animate-spin" } else { "h-5 w-5" } />
                    </button>
                </div>

                <div class="overflow-auto w-full flex-1">
                    <table class="table table-zebra w-full">
                        <thead>
                            <tr>
                                <th>"上游"</th>
                                <th>"目标"</th>
                                <th class="hidden md:table-cell">"触发模式"</th>
                                <th class="hidden md:table-cell">"下次检查"</th>
                                <th class="hidden lg:table-cell">"密钥"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <Show when=move || total_monitors() == 0 && !store.loading.get()>
                                <tr>
                                    <td colspan="5" class="text-center py-8 text-base-content/50">
                                        "未配置监控。添加一个以开始。"
                                    </td>
                                </tr>
                            </Show>
                                <Show when=move || store.loading.get() && total_monitors() == 0>
                                <tr>
                                    <td colspan="5" class="text-center py-8 text-base-content/50">
                                        <span class="loading loading-spinner loading-md"></span> " 加载中..."
                                    </td>
                                </tr>
                            </Show>
                            <For
                                each=move || store.projects.get()
                                key=|p| {
                                    match &p.state {
                                        MonitorState::Paused => format!("{}|paused", p.unique_key),
                                        MonitorState::Running { next_check_at } => {
                                            format!("{}|running|{}", p.unique_key, next_check_at.as_millis_i64())
                                        }
                                    }
                                }
                                children=move |project| {
                                    view! { <ProjectRow project=project /> }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
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
fn ProjectRow(project: ProjectConfig) -> impl IntoView {
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
                let secs = (*next_check_at - now).as_secs() as i64;
                format_countdown(secs)
            }
        }
    };

    let (id_pause, id_check, id_del) = (id.clone(), id.clone(), id.clone());

    view! {
        <tr
            class:opacity-50=is_paused
            class:grayscale=is_paused
            class:bg-base-200=is_paused
        >
            <td>
                <div class="flex items-center gap-2 font-mono text-sm font-bold">
                    <Github attr:class="h-4 w-4 opacity-50" />
                    {display.upstream}
                    <Show when=move || is_paused>
                        <span class="badge badge-warning badge-sm gap-1">
                            <Pause attr:class="h-3 w-3" /> "已暂停"
                        </span>
                    </Show>
                </div>
            </td>
            <td>
                <div class="flex items-center gap-2 font-mono text-sm opacity-70">
                    <GitFork attr:class="h-4 w-4 opacity-50" />
                    {display.target}
                </div>
            </td>
            <td class="hidden md:table-cell">
                <div class="badge badge-accent badge-outline">
                    {display.mode}
                </div>
            </td>
            <td class="hidden md:table-cell">
                <div class=move || {
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
                }>
                    <Clock attr:class="h-3 w-3 mr-1" />
                    {countdown_text}
                </div>
            </td>
            <td class="hidden lg:table-cell font-mono text-xs opacity-50">
                {display.secret}
            </td>
            <td>
                <div class="dropdown dropdown-end">
                    <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square">
                        <MoreHorizontal attr:class="h-4 w-4" />
                    </div>
                    <ul tabindex="0" class="dropdown-content z-[1] menu p-2 shadow bg-base-200 rounded-box w-52">
                        <li>
                            <a on:click=move |_| store.switch_monitor.run((id_pause.clone(), !is_paused))>
                                <Show when=move || is_paused
                                        fallback=|| view! { <Pause attr:class="mr-2 h-4 w-4" /> "暂停监控" }>
                                        <Play attr:class="mr-2 h-4 w-4" /> "恢复监控"
                                </Show>
                            </a>
                        </li>
                        <li>
                            <a on:click=move |_| store.trigger_check.run(id_check.clone())>
                                <RefreshCw attr:class="mr-2 h-4 w-4" /> "立即触发检查"
                            </a>
                        </li>
                        <li>
                            <a on:click=move |_| store.delete_project.run(id_del.clone()) class="text-error hover:bg-error/10">
                                <Trash2 attr:class="mr-2 h-4 w-4" />
                                "删除"
                            </a>
                        </li>
                    </ul>
                </div>
            </td>
        </tr>
    }
}
