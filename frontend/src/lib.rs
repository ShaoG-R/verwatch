mod api;
mod auth;
mod web;
mod components {
    mod add_project_dialog;
    pub mod dashboard;
    mod icons;
    pub mod login;
}

use crate::auth::{AuthContext, AuthState, init_auth};
use crate::components::dashboard::DashboardPage;
use crate::components::login::LoginPage;
use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    let (auth_state, set_auth) = signal(AuthState::default());
    provide_context(AuthContext(auth_state, set_auth));

    // Initialize authentication from local storage
    init_auth(set_auth);

    view! {
        <Router>
            <Routes fallback=|| "Not Found">
                <Route path=path!("/") view=LoginPage/>
                <Route path=path!("/dashboard") view=DashboardPage/>
            </Routes>
        </Router>
    }
}
