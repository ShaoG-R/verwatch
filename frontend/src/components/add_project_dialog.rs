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
use web_sys::HtmlDialogElement;

use crate::components::icons::Plus;
use silex::prelude::*;
use verwatch_shared::CreateProjectRequest;

/// 添加项目对话框组件
///
/// 职责：
/// - 模态框的开关控制
/// - 协调子组件
/// - 处理表单提交
#[component]
pub fn AddProjectDialog(#[prop(into)] on_add: Callback<CreateProjectRequest>) -> impl View {
    // 模态框状态
    let (open, set_open) = signal(false);
    let (loading, set_loading) = signal(false);
    let dialog_ref = NodeRef::<HtmlDialogElement>::new();

    // 初始化聚合状态
    let form_state = FormState::new();

    // 模态框同步 Effect
    Effect::new(move |_| {
        let is_open = open.get();
        if let Some(dialog) = dialog_ref.get() {
            if is_open {
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
    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        set_loading.set(true);

        let req = form_state.to_request();
        on_add.call(req);

        set_open.set(false);
        set_loading.set(false);
        form_state.reset();
    };

    div![
        // 触发按钮
        button((Plus().style("height: 16px; width: 16px;"), " 添加监控"))
            .class("btn btn-primary gap-2")
            .on(event::click, move |_| set_open.set(true)),
        // 模态框内容
        dialog((
            div![
                h3("添加新监控").class("font-bold text-lg"),
                p("配置要监控的上游仓库。").class("py-4 text-base-content/70"),
                form((
                    // 组合子组件
                    BasicInfoForm().state(form_state),
                    TimeConfigSection().state(form_state),
                    div![
                        button("取消")
                            .type_("button")
                            .class("btn btn-ghost")
                            .on(event::click, move |_| set_open.set(false)),
                        button(Dynamic::bind(loading, |is_loading| {
                            if is_loading {
                                div![span(()).class("loading loading-spinner"), " 添加中..."]
                                    .into_any()
                            } else {
                                span("添加监控").into_any()
                            }
                        }))
                        .type_("submit")
                        .disabled(loading)
                        .class("btn btn-primary")
                    ]
                    .class("modal-action")
                ))
                .class("space-y-4")
                .on(event::submit, on_submit)
            ]
            .class("modal-box"),
            form(button("close"))
                .attr("method", "dialog")
                .class("modal-backdrop")
        ))
        .class("modal")
        .node_ref(dialog_ref)
        .on_untyped("close", move |ev: web_sys::Event| {
            ev.prevent_default();
            set_open.set(false);
        })
    ]
}
