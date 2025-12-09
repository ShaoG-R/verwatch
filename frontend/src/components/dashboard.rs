use crate::auth::{AuthContext, logout, use_auth};
use crate::components::add_project_dialog::AddProjectDialog;
use crate::components::icons::*;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;
use verwatch_shared::{CreateProjectRequest, ProjectConfig};

#[component]
pub fn DashboardPage() -> impl IntoView {
    let AuthContext(auth_state, set_auth) = use_auth();
    let navigate = use_navigate();

    let (projects, set_projects) = signal(Vec::<ProjectConfig>::new());
    let (loading_projects, set_loading_projects) = signal(true);
    let (notification, set_notification) = signal(Option::<(String, bool)>::None);

    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            let state = auth_state.get();
            if !state.is_loading && !state.is_authenticated {
                navigate("/", Default::default());
            }
        }
    });

    let load_projects = move || {
        let state = auth_state.get();
        if let Some(api) = state.api.as_ref() {
            let api = api.clone();
            set_loading_projects.set(true);
            spawn_local(async move {
                match api.get_projects().await {
                    Ok(data) => set_projects.set(data),
                    Err(e) => set_notification.set(Some((format!("加载项目失败: {}", e), true))),
                }
                set_loading_projects.set(false);
            });
        }
    };

    Effect::new(move |_| {
        let state = auth_state.get();
        if state.is_authenticated && !state.is_loading {
            load_projects();
        }
    });

    let handle_add_project = move |req: CreateProjectRequest| {
        let state = auth_state.get();
        if let Some(api) = state.api.as_ref() {
            let api = api.clone();
            spawn_local(async move {
                match api.add_project(req).await {
                    Ok(_) => {
                        set_notification.set(Some(("监控添加成功".to_string(), false)));
                        load_projects();
                    }
                    Err(e) => set_notification.set(Some((format!("添加监控失败: {}", e), true))),
                }
            });
        }
    };

    let handle_delete = move |id: String| {
        let state = auth_state.get();
        if let Some(api) = state.api.as_ref() {
            let api = api.clone();
            spawn_local(async move {
                match api.delete_project(id.clone()).await {
                    Ok(deleted) => {
                        let msg = if deleted {
                            "监控已删除"
                        } else {
                            "监控不存在 (已清理)"
                        };
                        set_notification.set(Some((msg.to_string(), false)));
                        set_projects.update(|list| list.retain(|p| p.unique_key != id));
                    }
                    Err(e) => set_notification.set(Some((format!("删除监控失败: {}", e), true))),
                }
            });
        }
    };

    let handle_toggle_pause = move |id: String| {
        let state = auth_state.get();
        if let Some(api) = state.api.as_ref() {
            let api = api.clone();
            spawn_local(async move {
                match api.toggle_pause_project(id.clone()).await {
                    Ok(new_paused_state) => {
                        set_notification.set(Some((
                            if new_paused_state {
                                "监控已暂停".to_string()
                            } else {
                                "监控已恢复".to_string()
                            },
                            false,
                        )));
                        set_projects.update(|list| {
                            if let Some(p) = list.iter_mut().find(|p| p.unique_key == id) {
                                p.paused = new_paused_state;
                            }
                        });
                    }
                    Err(e) => {
                        set_notification.set(Some((format!("切换状态失败: {}", e), true)));
                    }
                }
            });
        }
    };

    let on_logout = move |_| {
        logout(set_auth);
        navigate("/", Default::default());
    };

    Effect::new(move |_| {
        if notification.get().is_some() {
            set_timeout(
                move || set_notification.set(None),
                std::time::Duration::from_secs(3),
            );
        }
    });

    let total_monitors = move || projects.with(|p| p.len());
    let backend_url = Signal::derive(move || auth_state.get().backend_url);

    view! {
        <div class="h-screen bg-base-200 p-4 md:p-8 font-sans flex flex-col overflow-hidden">
            <div class="max-w-7xl mx-auto w-full flex-1 flex flex-col gap-8 min-h-0">
                <NotificationToast notification=notification />

                <DashboardNavbar
                    backend_url=backend_url
                    on_add_project=Callback::new(handle_add_project)
                    on_logout=Callback::new(on_logout)
                />

                <DashboardStats total_monitors=Signal::derive(total_monitors) />

                <ProjectsTable
                    projects=projects
                    loading=loading_projects
                    on_refresh=Callback::new(move |_| load_projects())
                    on_toggle_pause=Callback::new(handle_toggle_pause)
                    on_delete=Callback::new(handle_delete)
                />
            </div>
        </div>
    }
}

#[component]
fn NotificationToast(notification: ReadSignal<Option<(String, bool)>>) -> impl IntoView {
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
    on_add_project: Callback<CreateProjectRequest>,
    on_logout: Callback<leptos::ev::MouseEvent>,
) -> impl IntoView {
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
                <AddProjectDialog on_add=move |req| on_add_project.run(req) />
                <button on:click=move |e| on_logout.run(e) class="btn btn-outline btn-error gap-2">
                    <LogOut attr:class="h-4 w-4" /> "断开连接"
                </button>
            </div>
        </div>
    }
}

#[component]
fn DashboardStats(total_monitors: Signal<usize>) -> impl IntoView {
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
fn ProjectsTable(
    projects: ReadSignal<Vec<ProjectConfig>>,
    loading: ReadSignal<bool>,
    on_refresh: Callback<()>,
    on_toggle_pause: Callback<String>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let total_monitors = move || projects.with(|p| p.len());

    view! {
        <div class="card bg-base-100 shadow-xl flex-1 flex flex-col min-h-0">
            <div class="card-body p-0 flex flex-col h-full overflow-hidden">
                <div class="flex items-center justify-between p-6 pb-2 flex-none">
                    <div>
                        <h3 class="card-title">"活跃监控"</h3>
                        <p class="text-base-content/70 text-sm">"管理您的仓库监控列表。目前共有 " {total_monitors} " 个监控项。"</p>
                    </div>
                    <button on:click=move |_| on_refresh.run(()) disabled=move || loading.get() class="btn btn-ghost btn-circle">
                        <RefreshCw attr:class=move || if loading.get() { "h-5 w-5 animate-spin" } else { "h-5 w-5" } />
                    </button>
                </div>

                <div class="overflow-auto w-full flex-1">
                    <table class="table table-zebra w-full">
                        <thead>
                            <tr>
                                <th>"上游"</th>
                                <th>"目标"</th>
                                <th class="hidden md:table-cell">"触发模式"</th>
                                <th class="hidden md:table-cell">"密钥"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <Show when=move || total_monitors() == 0 && !loading.get()>
                                <tr>
                                    <td colspan="5" class="text-center py-8 text-base-content/50">
                                        "未配置监控。添加一个以开始。"
                                    </td>
                                </tr>
                            </Show>
                                <Show when=move || loading.get() && total_monitors() == 0>
                                <tr>
                                    <td colspan="5" class="text-center py-8 text-base-content/50">
                                        <span class="loading loading-spinner loading-md"></span> " 加载中..."
                                    </td>
                                </tr>
                            </Show>
                            <For
                                each=move || projects.get()
                                key=|p| format!("{}|{}", p.unique_key, p.paused)
                                children=move |project| {
                                    view! {
                                        <ProjectRow
                                            project=project
                                            on_toggle_pause=on_toggle_pause
                                            on_delete=on_delete
                                        />
                                    }
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
            upstream: format!("{} / {}", p.base.upstream_owner, p.base.upstream_repo),
            target: format!("{} / {}", p.base.my_owner, p.base.my_repo),
            mode: format!("{:?}", p.base.comparison_mode),
            secret: p
                .base
                .dispatch_token_secret
                .clone()
                .unwrap_or("全局".to_string()),
        }
    }
}

#[component]
fn ProjectRow(
    project: ProjectConfig,
    on_toggle_pause: Callback<String>,
    on_delete: Callback<String>,
) -> impl IntoView {
    let id = project.unique_key.clone();
    let is_paused = project.paused;
    let display = ProjectRowDisplay::from(&project);

    // 预先克隆 ID 用于回调
    let (id_pause, id_del) = (id.clone(), id.clone());

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
            <td class="hidden md:table-cell font-mono text-xs opacity-50">
                {display.secret}
            </td>
            <td>
                <div class="dropdown dropdown-end">
                    <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square">
                        <MoreHorizontal attr:class="h-4 w-4" />
                    </div>
                    <ul tabindex="0" class="dropdown-content z-[1] menu p-2 shadow bg-base-200 rounded-box w-52">
                        <li>
                            <a on:click=move |_| on_toggle_pause.run(id_pause.clone())>
                                <Show when=move || is_paused
                                        fallback=|| view! { <Pause attr:class="mr-2 h-4 w-4" /> "暂停监控" }>
                                        <Play attr:class="mr-2 h-4 w-4" /> "恢复监控"
                                </Show>
                            </a>
                        </li>
                        <li>
                            <a on:click=move |_| on_delete.run(id_del.clone()) class="text-error hover:bg-error/10">
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
