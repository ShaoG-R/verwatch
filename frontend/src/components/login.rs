//! 登录页面组件
//!
//! 纯粹的 UI 组件，不直接处理路由逻辑。
//! 导航由路由服务根据认证状态变化自动处理。

use crate::auth::{login, use_auth};
use crate::components::icons::ShieldCheck;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth();

    // 从认证状态获取初始 URL
    let initial_url = auth.state.get_untracked().backend_url;

    let (url, set_url) = signal(initial_url);
    let (secret, set_secret) = signal(String::new());
    let (is_submitting, set_is_submitting) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);

    // 检查加载状态以显示加载指示器
    let is_loading = move || auth.state.get().is_loading;

    view! {
        <Show when=move || !is_loading() fallback=|| view! {
            <div class="flex items-center justify-center min-h-screen">
                <span class="loading loading-spinner loading-lg text-primary"></span>
            </div>
        }>
            {
                let on_submit = move |ev: leptos::web_sys::SubmitEvent| {
                    ev.prevent_default();
                    if url.get().is_empty() || secret.get().is_empty() {
                        set_error_msg.set(Some("请填写所有字段".to_string()));
                        return;
                    }

                    set_is_submitting.set(true);
                    set_error_msg.set(None);

                    // 在进入异步上下文前获取值（避免在非响应式上下文中访问信号）
                    let url_value = url.get_untracked();
                    let secret_value = secret.get_untracked();

                    spawn_local(async move {
                        let success = login(&auth, url_value, secret_value).await;
                        if !success {
                            set_error_msg.set(Some("连接失败。请检查 URL 和密钥。".to_string()));
                        }
                        // 成功时不需要手动导航 - 路由服务会监听认证状态变化并自动重定向
                        set_is_submitting.set(false);
                    });
                };

                view! {
                    <div class="hero min-h-screen bg-base-200">
                        <div class="hero-content flex-col w-full max-w-md">
                            <div class="text-center mb-4">
                                <div class="flex flex-col items-center gap-2">
                                    <div class="p-3 bg-primary/10 rounded-2xl text-primary">
                                        <ShieldCheck attr:class="h-8 w-8" />
                                    </div>
                                    <h1 class="text-3xl font-bold">"VerWatch 面板"</h1>
                                    <p class="text-base-content/70">
                                        "输入您的 Worker 凭证以继续"
                                    </p>
                                </div>
                            </div>

                            <div class="card shrink-0 w-full shadow-2xl bg-base-100">
                                <form class="card-body" on:submit=on_submit>
                                    <Show when=move || error_msg.get().is_some()>
                                        <div role="alert" class="alert alert-error text-sm py-2">
                                            <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
                                            <span>{move || error_msg.get().unwrap()}</span>
                                        </div>
                                    </Show>

                                    <div class="form-control">
                                        <label class="label" for="url">
                                            <span class="label-text">"后端 URL"</span>
                                        </label>
                                        <input
                                            id="url"
                                            type="text"
                                            placeholder="https://verwatch.workers.dev"
                                            on:input=move |ev| set_url.set(event_target_value(&ev))
                                            prop:value=url
                                            class="input input-bordered"
                                            required
                                        />
                                    </div>
                                    <div class="form-control">
                                        <label class="label" for="secret">
                                            <span class="label-text">"管理密钥"</span>
                                        </label>
                                        <input
                                            id="secret"
                                            type="password"
                                            placeholder="••••••••"
                                            on:input=move |ev| set_secret.set(event_target_value(&ev))
                                            prop:value=secret
                                            class="input input-bordered"
                                            required
                                        />
                                    </div>
                                    <div class="form-control mt-6">
                                        <button class="btn btn-primary" disabled=move || is_submitting.get()>
                                            {move || if is_submitting.get() {
                                                view! { <span class="loading loading-spinner"></span> "连接中..." }.into_any()
                                            } else {
                                                "连接到控制台".into_any()
                                            }}
                                        </button>
                                    </div>
                                </form>
                            </div>
                        </div>
                    </div>
                }
            }
        </Show>
    }
}
