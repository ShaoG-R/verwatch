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
    let (notification, set_notification) = signal(Option::<(String, bool)>::None); // 消息内容, 是否出错

    // 如果未认证则重定向
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
                    Err(e) => {
                        set_notification.set(Some((format!("加载项目失败: {}", e), true)));
                    }
                }
                set_loading_projects.set(false);
            });
        }
    };

    // 初始加载
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
                    Err(e) => {
                        set_notification.set(Some((format!("添加监控失败: {}", e), true)));
                    }
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
                    Ok(_) => {
                        set_notification.set(Some(("监控已删除".to_string(), false)));
                        set_projects.update(|list| list.retain(|p| p.unique_key != id));
                    }
                    Err(e) => {
                        set_notification.set(Some((format!("删除监控失败: {}", e), true)));
                    }
                }
            });
        }
    };

    let on_logout = move |_| {
        logout(set_auth);
        navigate("/", Default::default());
    };

    // 3秒后清除通知
    Effect::new(move |_| {
        if notification.get().is_some() {
            set_timeout(
                move || set_notification.set(None),
                std::time::Duration::from_secs(3),
            );
        }
    });

    // 统计数据的派生值
    let total_monitors = move || projects.with(|p| p.len());

    view! {
        <div class="min-h-screen bg-base-200 p-4 md:p-8 font-sans">
            <div class="max-w-7xl mx-auto space-y-8">
                // 通知提示框
                <Show when=move || notification.get().is_some()>
                    <div class="toast toast-top toast-end z-50">
                        <div class=move || {
                            let (_, is_err) = notification.get().unwrap();
                            if is_err {
                                "alert alert-error shadow-lg"
                            } else {
                                "alert alert-success shadow-lg"
                            }
                        }>
                            <span>{move || notification.get().unwrap().0}</span>
                        </div>
                    </div>
                </Show>

                <div class="navbar bg-base-100 rounded-box shadow-xl">
                    <div class="flex-1 gap-2">
                        <Radio attr:class="text-primary h-6 w-6 animate-pulse" />
                        <a class="btn btn-ghost text-xl">"VerWatch 控制面板"</a>
                        <span class="badge badge-neutral hidden md:inline-flex">
                            "已连接至 " {move || auth_state.get().backend_url}
                        </span>
                    </div>
                    <div class="flex-none gap-2">
                        <AddProjectDialog on_add=handle_add_project />
                        <button on:click=on_logout class="btn btn-outline btn-error gap-2">
                            <LogOut attr:class="h-4 w-4" /> "断开连接"
                        </button>
                    </div>
                </div>

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

                <div class="card bg-base-100 shadow-xl">
                    <div class="card-body p-0">
                        <div class="flex items-center justify-between p-6 pb-2">
                            <div>
                                <h3 class="card-title">"活跃监控"</h3>
                                <p class="text-base-content/70 text-sm">"管理您的仓库监控列表。"</p>
                            </div>
                            <button on:click=move |_| load_projects() disabled=move || loading_projects.get() class="btn btn-ghost btn-circle">
                                <RefreshCw attr:class=move || if loading_projects.get() { "h-5 w-5 animate-spin" } else { "h-5 w-5" } />
                            </button>
                        </div>

                        <div class="overflow-x-auto w-full">
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
                                    <Show when=move || total_monitors() == 0 && !loading_projects.get()>
                                        <tr>
                                            <td colspan="5" class="text-center py-8 text-base-content/50">
                                                "未配置监控。添加一个以开始。"
                                            </td>
                                        </tr>
                                    </Show>
                                     <Show when=move || loading_projects.get() && total_monitors() == 0>
                                        <tr>
                                            <td colspan="5" class="text-center py-8 text-base-content/50">
                                                <span class="loading loading-spinner loading-md"></span> " 加载中..."
                                            </td>
                                        </tr>
                                    </Show>
                                    <For
                                        each=move || projects.get()
                                        key=|p| p.unique_key.clone()
                                        children=move |project| {
                                            let id = project.unique_key.clone();
                                            view! {
                                                 <tr>
                                                    <td>
                                                        <div class="flex items-center gap-2 font-mono text-sm font-bold">
                                                            <Github attr:class="h-4 w-4 opacity-50" />
                                                            {project.base.upstream_owner} "/" {project.base.upstream_repo}
                                                        </div>
                                                    </td>
                                                    <td>
                                                        <div class="flex items-center gap-2 font-mono text-sm opacity-70">
                                                            <GitFork attr:class="h-4 w-4 opacity-50" />
                                                            {project.base.my_owner} "/" {project.base.my_repo}
                                                        </div>
                                                    </td>
                                                    <td class="hidden md:table-cell">
                                                        <div class="badge badge-accent badge-outline">
                                                            {format!("{:?}", project.base.comparison_mode)}
                                                        </div>
                                                    </td>
                                                    <td class="hidden md:table-cell font-mono text-xs opacity-50">
                                                        {project.base.dispatch_token_secret.clone().unwrap_or("全局".to_string())}
                                                    </td>
                                                    <td>
                                                        <div class="dropdown dropdown-end">
                                                            <div tabindex="0" role="button" class="btn btn-ghost btn-sm btn-square">
                                                                <MoreHorizontal attr:class="h-4 w-4" />
                                                            </div>
                                                            <ul tabindex="0" class="dropdown-content z-[1] menu p-2 shadow bg-base-200 rounded-box w-52">
                                                                <li>
                                                                    <a on:click=move |_| handle_delete(id.clone()) class="text-error hover:bg-error/10">
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
                                    />
                                </tbody>
                            </table>
                         </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
