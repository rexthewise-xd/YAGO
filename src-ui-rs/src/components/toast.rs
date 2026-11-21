use dioxus::prelude::*;

use crate::icons::{IconName, IconSvg};
use crate::state::{Store, Toast, ToastKind};

#[component]
pub fn Toasts() -> Element {
    let store = use_context::<Store>();
    let toasts = (store.toasts)();

    rsx! {
        div { class: "toasts",
            for toast in toasts.iter() {
                ToastItem { key: "{toast.id}", toast: toast.clone() }
            }
        }
    }
}

#[component]
fn ToastItem(toast: Toast) -> Element {
    let mut store = use_context::<Store>();
    let id = toast.id;
    let (icon, class) = match toast.kind {
        ToastKind::Info => (IconName::Info, "toast info"),
        ToastKind::Success => (IconName::Check, "toast success"),
        ToastKind::Error => (IconName::Warning, "toast error"),
    };

    rsx! {
        div { class: "{class}",
            IconSvg { name: icon, size: 15 }
            span { class: "toast-msg", "{toast.message}" }
            button {
                class: "toast-x",
                onclick: move |_| {
                    store.toasts.write().retain(|t| t.id != id);
                },
                IconSvg { name: IconName::Close, size: 12 }
            }
        }
    }
}