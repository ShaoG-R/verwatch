use crate::api::VerWatchApi;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;

const STORAGE_URL_KEY: &str = "verwatch_url";
const STORAGE_SECRET_KEY: &str = "verwatch_secret";

#[derive(Clone, Debug, Default)]
pub struct AuthState {
    pub api: Option<VerWatchApi>,
    pub is_authenticated: bool,
    pub is_loading: bool,
    pub backend_url: String,
}

#[derive(Clone, Copy)]
pub struct AuthContext(pub ReadSignal<AuthState>, pub WriteSignal<AuthState>);

pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>().expect("AuthContext should be provided")
}

pub fn init_auth(set_auth: WriteSignal<AuthState>) {
    // Try to load from local storage
    if let (Ok(url), Ok(secret)) = (
        LocalStorage::get::<String>(STORAGE_URL_KEY),
        LocalStorage::get::<String>(STORAGE_SECRET_KEY),
    ) {
        let api = VerWatchApi::new(url.clone(), secret);
        set_auth.update(|state| {
            state.api = Some(api);
            state.backend_url = url;
            state.is_authenticated = true;
            state.is_loading = false;
        });
    } else {
        set_auth.update(|state| {
            state.is_loading = false;
        });
    }
}

pub async fn login(set_auth: WriteSignal<AuthState>, url: String, secret: String) -> bool {
    let api = VerWatchApi::new(url.clone(), secret.clone());
    if api.get_projects().await.is_ok() {
        let _ = LocalStorage::set(STORAGE_URL_KEY, &url);
        let _ = LocalStorage::set(STORAGE_SECRET_KEY, &secret);
        set_auth.update(|state| {
            state.api = Some(api);
            state.backend_url = url;
            state.is_authenticated = true;
        });
        true
    } else {
        false
    }
}

pub fn logout(set_auth: WriteSignal<AuthState>) {
    let _ = LocalStorage::delete(STORAGE_URL_KEY);
    let _ = LocalStorage::delete(STORAGE_SECRET_KEY);
    set_auth.update(|state| {
        state.api = None;
        state.is_authenticated = false;
        state.backend_url = String::new();
    });
    // Navigation should be handled by the component listening to auth state
}
