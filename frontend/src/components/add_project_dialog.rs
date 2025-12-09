use crate::components::icons::Plus;
use leptos::prelude::*;
use verwatch_shared::{ComparisonMode, CreateProjectRequest};

#[component]
pub fn AddProjectDialog(#[prop(into)] on_add: Callback<CreateProjectRequest>) -> impl IntoView {
    let (open, set_open) = signal(false);
    let (loading, set_loading) = signal(false);
    let dialog_ref = NodeRef::<leptos::html::Dialog>::new();

    // 表单字段
    let (u_owner, set_u_owner) = signal(String::new());
    let (u_repo, set_u_repo) = signal(String::new());
    let (m_owner, set_m_owner) = signal(String::new());
    let (m_repo, set_m_repo) = signal(String::new());
    let (comp_mode, set_comp_mode) = signal(ComparisonMode::PublishedAt);
    let (token_secret, set_token_secret) = signal(String::new());

    let reset_form = move || {
        set_u_owner.set(String::new());
        set_u_repo.set(String::new());
        set_m_owner.set(String::new());
        set_m_repo.set(String::new());
        set_comp_mode.set(ComparisonMode::PublishedAt);
        set_token_secret.set(String::new());
    };

    Effect::new(move |_| {
        if let Some(dialog) = dialog_ref.get() {
            if open.get() {
                if !dialog.open() {
                    let _ = dialog.show_modal();
                }
            } else {
                if dialog.open() {
                    dialog.close();
                }
            }
        }
    });

    let on_submit = move |ev: leptos::web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);

        // 准备请求
        let secret = token_secret.get();
        let secret_opt = if secret.trim().is_empty() {
            None
        } else {
            Some(secret)
        };

        let req = CreateProjectRequest {
            upstream_owner: u_owner.get(),
            upstream_repo: u_repo.get(),
            my_owner: m_owner.get(),
            my_repo: m_repo.get(),
            comparison_mode: comp_mode.get(),
            dispatch_token_secret: secret_opt,
        };

        on_add.run(req);
        set_open.set(false);
        set_loading.set(false);
        reset_form();
    };

    view! {
        // 触发按钮
        <button
            class="btn btn-primary gap-2"
            on:click=move |_| set_open.set(true)
        >
            <Plus attr:class="h-4 w-4" /> "添加监控"
        </button>

        // 模态框内容
        <dialog class="modal" node_ref=dialog_ref on:close=move |_| set_open.set(false)>
             <div class="modal-box">
                <h3 class="font-bold text-lg">"添加新监控"</h3>
                <p class="py-4 text-base-content/70">"配置要监控的上游仓库。"</p>

                <form on:submit=on_submit class="space-y-4">
                    <div class="grid grid-cols-2 gap-4">
                        <div class="form-control">
                            <label for="u_owner" class="label">
                                <span class="label-text">"上游所有者"</span>
                            </label>
                            <input id="u_owner" required
                                type="text"
                                placeholder="fail2ban"
                                on:input=move |ev| set_u_owner.set(event_target_value(&ev))
                                prop:value=u_owner
                                class="input input-bordered w-full"
                            />
                        </div>
                        <div class="form-control">
                            <label for="u_repo" class="label">
                                <span class="label-text">"上游仓库名"</span>
                            </label>
                            <input id="u_repo" required
                                type="text"
                                placeholder="fail2ban"
                                on:input=move |ev| set_u_repo.set(event_target_value(&ev))
                                prop:value=u_repo
                                class="input input-bordered w-full"
                            />
                        </div>
                    </div>

                    <div class="grid grid-cols-2 gap-4">
                        <div class="form-control">
                            <label for="m_owner" class="label">
                                <span class="label-text">"我的用户名"</span>
                            </label>
                            <input id="m_owner" required
                                type="text"
                                placeholder="my-user"
                                on:input=move |ev| set_m_owner.set(event_target_value(&ev))
                                prop:value=m_owner
                                class="input input-bordered w-full"
                            />
                        </div>
                        <div class="form-control">
                            <label for="m_repo" class="label">
                                <span class="label-text">"我的仓库名"</span>
                            </label>
                            <input id="m_repo" required
                                type="text"
                                placeholder="my-fork"
                                on:input=move |ev| set_m_repo.set(event_target_value(&ev))
                                prop:value=m_repo
                                class="input input-bordered w-full"
                            />
                        </div>
                    </div>

                    <div class="form-control">
                        <label class="label">
                            <span class="label-text">"比对模式"</span>
                        </label>
                        <select
                            class="select select-bordered w-full"
                            on:change=move |ev| {
                                let val = event_target_value(&ev);
                                if val == "updated_at" { set_comp_mode.set(ComparisonMode::UpdatedAt); }
                                else { set_comp_mode.set(ComparisonMode::PublishedAt); }
                            }
                        >
                            <option value="published_at" selected=move || comp_mode.get() == ComparisonMode::PublishedAt>"发布时间 (推荐)"</option>
                            <option value="updated_at" selected=move || comp_mode.get() == ComparisonMode::UpdatedAt>"更新时间"</option>
                        </select>
                    </div>

                    <div class="form-control">
                        <label for="token_secret" class="label">
                            <span class="label-text">"Token 密钥名称 (可选)"</span>
                        </label>
                        <input id="token_secret"
                            type="text"
                            placeholder="MY_CUSTOM_TOKEN"
                            on:input=move |ev| set_token_secret.set(event_target_value(&ev))
                            prop:value=token_secret
                            class="input input-bordered w-full"
                        />
                        <label class="label">
                            <span class="label-text-alt text-base-content/50">"留空以使用全局 MY_GITHUB_PAT"</span>
                        </label>
                    </div>

                    <div class="modal-action">
                         <button type="button" class="btn btn-ghost" on:click=move |_| set_open.set(false)>"取消"</button>
                         <button type="submit" disabled=move || loading.get() class="btn btn-primary">
                            {move || if loading.get() {
                                view! { <span class="loading loading-spinner"></span> "添加中..." }.into_any()
                            } else {
                                "添加监控".into_any()
                            }}
                         </button>
                    </div>
                </form>
            </div>
            <form method="dialog" class="modal-backdrop">
                 <button>"close"</button>
            </form>
        </dialog>
    }
}
