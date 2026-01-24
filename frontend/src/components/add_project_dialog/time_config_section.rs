//! 时间配置表单组件
//!
//! 负责处理与时间配置相关的特定 UI 逻辑，
//! 包括条件渲染和单位选择（小时/分钟）。

use silex::prelude::*;

use super::form_state::FormState;

/// 时间配置表单组件
///
/// 显示自定义时间配置开关，以及检查间隔和重试间隔的输入。
#[component]
pub fn TimeConfigSection(state: FormState) -> impl View {
    div![
        // 自定义时间配置开关
        div![
            label((
                span("自定义时间配置").class("label-text font-bold"),
                input()
                    .type_("checkbox")
                    .class("toggle toggle-primary")
                    .prop("checked", state.use_custom_time)
                    .on(event::change, move |ev| state
                        .use_custom_time
                        .set(event_target_checked(&ev)))
            ))
            .class("label cursor-pointer")
        ]
        .class("form-control"),
        // 条件渲染：仅在启用自定义时间配置时显示
        Show::new(state.use_custom_time, move || {
            div![
                // 检查间隔输入
                div![
                    label(span("检查间隔").class("label-text")).class("label"),
                    div![
                        input()
                            .type_("number")
                            .attr("min", "1")
                            .required(true)
                            .class("input input-bordered join-item w-full")
                            .prop("value", state.check_interval_val)
                            .on(event::input, move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                    state.check_interval_val.set(val);
                                }
                            }),
                        select((
                            option("小时")
                                .value("hours")
                                .selected(state.check_interval_unit.map(|u| u == "hours")),
                            option("分钟")
                                .value("minutes")
                                .selected(state.check_interval_unit.map(|u| u == "minutes")),
                        ))
                        .class("select select-bordered join-item")
                        .on(event::change, move |ev| state
                            .check_interval_unit
                            .set(event_target_value(&ev)))
                    ]
                    .class("join")
                ]
                .class("form-control"),
                // 重试间隔输入
                div![
                    label(span("重试间隔 (秒)").class("label-text")).class("label"),
                    input()
                        .type_("number")
                        .attr("min", "1")
                        .required(true)
                        .class("input input-bordered w-full")
                        .prop("value", state.retry_interval_seconds)
                        .on(event::input, move |ev| {
                            if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                state.retry_interval_seconds.set(val);
                            }
                        })
                ]
                .class("form-control")
            ]
            .class("grid grid-cols-2 gap-4 bg-base-200 p-4 rounded-lg")
        })
    ]
}
