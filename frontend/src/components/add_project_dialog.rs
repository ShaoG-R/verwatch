//! 添加项目对话框组件
//!
//! 采用模块化架构重构，将原有巨石组件拆分为：
//! - `form_state`: 表单状态管理（数据持有、重置、转换）
//! - `basic_info_form`: 基础信息表单 UI
//! - `time_config_section`: 时间配置表单 UI
//!
//! 主组件仅负责模态框生命周期和提交动作的协调。

// Rust 2018 Edition 风格子模块声明
mod basic_info_form;
mod form_state;
mod time_config_section;

use basic_info_form::BasicInfoForm;
use form_state::FormState;
use time_config_section::TimeConfigSection;

use crate::components::icons::Plus;
use leptos::prelude::*;
use verwatch_shared::CreateProjectRequest;

/// 添加项目对话框组件
///
/// 职责：
/// - 模态框的开关控制
/// - 协调子组件
/// - 处理表单提交
#[component]
pub fn AddProjectDialog(#[prop(into)] on_add: Callback<CreateProjectRequest>) -> impl IntoView {
    // 模态框状态
    let (open, set_open) = signal(false);
    let (loading, set_loading) = signal(false);
    let dialog_ref = NodeRef::<leptos::html::Dialog>::new();

    // 初始化聚合状态
    let form_state = FormState::new();

    // 模态框同步 Effect
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

    // 提交处理（简化，逻辑移到了 FormState::to_request）
    let on_submit = move |ev: leptos::web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);

        let req = form_state.to_request();
        on_add.run(req);

        set_open.set(false);
        set_loading.set(false);
        form_state.reset();
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
                    // 组合子组件
                    <BasicInfoForm state=form_state />

                    <TimeConfigSection state=form_state />

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
