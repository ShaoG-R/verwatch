//! 基础信息表单组件
//!
//! 负责仓库名称、所有者、比对模式和 Token 配置的 UI 渲染。
//! 纯粹的表单输入渲染，职责单一。

use leptos::prelude::*;
use verwatch_shared::ComparisonMode;

use super::form_state::FormState;

/// 基础信息表单组件
///
/// 显示上游/本地仓库配置、比对模式和 Token 密钥输入。
#[component]
pub fn BasicInfoForm(state: FormState) -> impl IntoView {
    view! {
        // 上游仓库配置
        <div class="grid grid-cols-2 gap-4">
            <div class="form-control">
                <label for="u_owner" class="label">
                    <span class="label-text">"上游所有者"</span>
                </label>
                <input id="u_owner" required
                    type="text"
                    placeholder="fail2ban"
                    on:input=move |ev| state.u_owner.set(event_target_value(&ev))
                    prop:value=move || state.u_owner.get()
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
                    on:input=move |ev| state.u_repo.set(event_target_value(&ev))
                    prop:value=move || state.u_repo.get()
                    class="input input-bordered w-full"
                />
            </div>
        </div>

        // 我的仓库配置
        <div class="grid grid-cols-2 gap-4">
            <div class="form-control">
                <label for="m_owner" class="label">
                    <span class="label-text">"我的用户名"</span>
                </label>
                <input id="m_owner" required
                    type="text"
                    placeholder="my-user"
                    on:input=move |ev| state.m_owner.set(event_target_value(&ev))
                    prop:value=move || state.m_owner.get()
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
                    on:input=move |ev| state.m_repo.set(event_target_value(&ev))
                    prop:value=move || state.m_repo.get()
                    class="input input-bordered w-full"
                />
            </div>
        </div>

        // 比对模式选择
        <div class="form-control">
            <label class="label">
                <span class="label-text">"比对模式"</span>
            </label>
            <select
                class="select select-bordered w-full"
                on:change=move |ev| {
                    let val = event_target_value(&ev);
                    if val == "updated_at" {
                        state.comp_mode.set(ComparisonMode::UpdatedAt);
                    } else {
                        state.comp_mode.set(ComparisonMode::PublishedAt);
                    }
                }
            >
                <option
                    value="published_at"
                    selected=move || state.comp_mode.get() == ComparisonMode::PublishedAt
                >
                    "发布时间 (推荐)"
                </option>
                <option
                    value="updated_at"
                    selected=move || state.comp_mode.get() == ComparisonMode::UpdatedAt
                >
                    "更新时间"
                </option>
            </select>
        </div>

        // Token 密钥配置
        <div class="form-control">
            <label for="token_secret" class="label">
                <span class="label-text">"Token 密钥名称 (可选)"</span>
            </label>
            <input id="token_secret"
                type="text"
                placeholder="MY_CUSTOM_TOKEN"
                on:input=move |ev| state.token_secret.set(event_target_value(&ev))
                prop:value=move || state.token_secret.get()
                class="input input-bordered w-full"
            />
            <label class="label">
                <span class="label-text-alt text-base-content/50">"留空以使用全局 MY_GITHUB_PAT"</span>
            </label>
        </div>
    }
}
