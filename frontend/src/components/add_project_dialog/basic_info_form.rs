//! 基础信息表单组件
//!
//! 负责仓库名称、所有者、比对模式和 Token 配置的 UI 渲染。
//! 纯粹的表单输入渲染，职责单一。

use silex::prelude::*;
use verwatch_shared::ComparisonMode;

use super::form_state::FormState;

/// 基础信息表单组件
///
/// 显示上游/本地仓库配置、比对模式和 Token 密钥输入。
#[component]
pub fn BasicInfoForm(state: FormState) -> impl View {
    div![
        // 上游仓库配置
        div![
            div![
                label(span("上游所有者").class("label-text"))
                    .for_("u_owner")
                    .class("label"),
                input()
                    .id("u_owner")
                    .required(true)
                    .type_("text")
                    .placeholder("fail2ban")
                    .on(event::input, move |ev| state
                        .u_owner
                        .set(event_target_value(&ev)))
                    .prop("value", state.u_owner)
                    .class("input input-bordered w-full")
            ]
            .class("form-control"),
            div![
                label(span("上游仓库名").class("label-text"))
                    .for_("u_repo")
                    .class("label"),
                input()
                    .id("u_repo")
                    .required(true)
                    .type_("text")
                    .placeholder("fail2ban")
                    .on(event::input, move |ev| state
                        .u_repo
                        .set(event_target_value(&ev)))
                    .prop("value", state.u_repo)
                    .class("input input-bordered w-full")
            ]
            .class("form-control")
        ]
        .class("grid grid-cols-2 gap-4"),
        // 我的仓库配置
        div![
            div![
                label(span("我的用户名").class("label-text"))
                    .for_("m_owner")
                    .class("label"),
                input()
                    .id("m_owner")
                    .required(true)
                    .type_("text")
                    .placeholder("my-user")
                    .on(event::input, move |ev| state
                        .m_owner
                        .set(event_target_value(&ev)))
                    .prop("value", state.m_owner)
                    .class("input input-bordered w-full")
            ]
            .class("form-control"),
            div![
                label(span("我的仓库名").class("label-text"))
                    .for_("m_repo")
                    .class("label"),
                input()
                    .id("m_repo")
                    .required(true)
                    .type_("text")
                    .placeholder("my-fork")
                    .on(event::input, move |ev| state
                        .m_repo
                        .set(event_target_value(&ev)))
                    .prop("value", state.m_repo)
                    .class("input input-bordered w-full")
            ]
            .class("form-control")
        ]
        .class("grid grid-cols-2 gap-4"),
        // 比对模式选择
        div![
            label(span("比对模式").class("label-text")).class("label"),
            select((
                option("发布时间 (推荐)")
                    .value("published_at")
                    .selected(state.comp_mode.map(|m| m == ComparisonMode::PublishedAt)),
                option("更新时间")
                    .value("updated_at")
                    .selected(state.comp_mode.map(|m| m == ComparisonMode::UpdatedAt)),
            ))
            .class("select select-bordered w-full")
            .on(event::change, move |ev| {
                let val = event_target_value(&ev);
                if val == "updated_at" {
                    state.comp_mode.set(ComparisonMode::UpdatedAt);
                } else {
                    state.comp_mode.set(ComparisonMode::PublishedAt);
                }
            })
        ]
        .class("form-control"),
        // Token 密钥配置
        div![
            label(span("Token 密钥名称 (可选)").class("label-text"))
                .for_("token_secret")
                .class("label"),
            input()
                .id("token_secret")
                .type_("text")
                .placeholder("MY_CUSTOM_TOKEN")
                .on(event::input, move |ev| state
                    .token_secret
                    .set(event_target_value(&ev)))
                .prop("value", state.token_secret)
                .class("input input-bordered w-full"),
            label(
                span("留空以使用全局 MY_GITHUB_PAT").class("label-text-alt text-base-content/50")
            )
            .class("label")
        ]
        .class("form-control")
    ]
}
