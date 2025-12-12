//! 时间配置表单组件
//!
//! 负责处理与时间配置相关的特定 UI 逻辑，
//! 包括条件渲染和单位选择（小时/分钟）。

use leptos::prelude::*;

use super::form_state::FormState;

/// 时间配置表单组件
///
/// 显示自定义时间配置开关，以及检查间隔和重试间隔的输入。
#[component]
pub fn TimeConfigSection(state: FormState) -> impl IntoView {
    view! {
        // 自定义时间配置开关
        <div class="form-control">
            <label class="label cursor-pointer">
                <span class="label-text font-bold">"自定义时间配置"</span>
                <input type="checkbox" class="toggle toggle-primary"
                    prop:checked=move || state.use_custom_time.get()
                    on:change=move |ev| state.use_custom_time.set(event_target_checked(&ev))
                />
            </label>
        </div>

        // 条件渲染：仅在启用自定义时间配置时显示
        {move || if state.use_custom_time.get() {
            view! {
                <div class="grid grid-cols-2 gap-4 bg-base-200 p-4 rounded-lg">
                    // 检查间隔输入
                    <div class="form-control">
                        <label class="label">
                            <span class="label-text">"检查间隔"</span>
                        </label>
                        <div class="join">
                            <input type="number" min="1" required
                                class="input input-bordered join-item w-full"
                                prop:value=move || state.check_interval_val.get()
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                        state.check_interval_val.set(val);
                                    }
                                }
                            />
                            <select class="select select-bordered join-item"
                                on:change=move |ev| state.check_interval_unit.set(event_target_value(&ev))
                            >
                                <option
                                    value="hours"
                                    selected=move || state.check_interval_unit.get() == "hours"
                                >
                                    "小时"
                                </option>
                                <option
                                    value="minutes"
                                    selected=move || state.check_interval_unit.get() == "minutes"
                                >
                                    "分钟"
                                </option>
                            </select>
                        </div>
                    </div>
                    // 重试间隔输入
                    <div class="form-control">
                        <label class="label">
                            <span class="label-text">"重试间隔 (秒)"</span>
                        </label>
                        <input type="number" min="1" required
                            class="input input-bordered w-full"
                            prop:value=move || state.retry_interval_seconds.get()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                    state.retry_interval_seconds.set(val);
                                }
                            }
                        />
                    </div>
                </div>
            }.into_any()
        } else {
            view! { <></> }.into_any()
        }}
    }
}
