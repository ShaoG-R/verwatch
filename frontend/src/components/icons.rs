use leptos::attr::any_attribute::AnyAttribute;
use leptos::prelude::*;

#[component]
pub fn ShieldCheck(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}

            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"></path>
            <path d="m9 12 2 2 4-4"></path>
        </svg>
    }
}

#[component]
pub fn Radio(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9"></path>
            <path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"></path>
            <circle cx="12" cy="12" r="2"></circle>
            <path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"></path>
            <path d="M19.1 4.9C23 8.8 23 15.1 19.1 19"></path>
        </svg>
    }
}

#[component]
pub fn LogOut(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
            <polyline points="16 17 21 12 16 7"></polyline>
            <line x1="21" x2="9" y1="12" y2="12"></line>
        </svg>
    }
}

#[component]
pub fn Plus(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M5 12h14"></path>
            <path d="M12 5v14"></path>
        </svg>
    }
}

#[component]
pub fn MoreHorizontal(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <circle cx="12" cy="12" r="1"></circle>
            <circle cx="19" cy="12" r="1"></circle>
            <circle cx="5" cy="12" r="1"></circle>
        </svg>
    }
}

#[component]
pub fn Trash2(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M3 6h18"></path>
            <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path>
            <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path>
            <line x1="10" x2="10" y1="11" y2="17"></line>
            <line x1="14" x2="14" y1="11" y2="17"></line>
        </svg>
    }
}

#[component]
pub fn Github(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
           <path d="M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.403 5.403 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65-.17.6-.22 1.23-.15 1.85v4"></path>
           <path d="M9 18c-4.51 2-5-2-7-2"></path>
        </svg>
    }
}

#[component]
pub fn GitFork(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <circle cx="12" cy="18" r="3"></circle>
            <circle cx="6" cy="6" r="3"></circle>
            <circle cx="18" cy="6" r="3"></circle>
            <path d="M18 9v1a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V9"></path>
            <path d="M12 12v3"></path>
        </svg>
    }
}

#[component]
pub fn RefreshCw(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"></path>
            <path d="M21 3v5h-5"></path>
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"></path>
            <path d="M8 16H3v5"></path>
        </svg>
    }
}

#[component]
pub fn Pause(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <rect x="6" y="4" width="4" height="16"></rect>
            <rect x="14" y="4" width="4" height="16"></rect>
        </svg>
    }
}

#[component]
pub fn Play(#[prop(attrs)] attrs: Vec<AnyAttribute>) -> impl IntoView {
    view! {
        <svg
            {..attrs}
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <polygon points="5 3 19 12 5 21 5 3"></polygon>
        </svg>
    }
}
