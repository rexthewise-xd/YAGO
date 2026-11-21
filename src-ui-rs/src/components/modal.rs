use dioxus::prelude::*;

use crate::icons::{IconName, IconSvg};

#[component]
pub fn Modal(
    title: String,
    #[props(default)] subtitle: Option<String>,
    #[props(default)] wide: bool,
    onclose: EventHandler<()>,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| onclose.call(()),
            div {
                class: if wide { "modal modal-wide" } else { "modal" },
                onclick: move |evt| evt.stop_propagation(),
                div { class: "modal-head",
                    div { class: "modal-titles",
                        h2 { class: "modal-title", "{title}" }
                        if let Some(sub) = subtitle {
                            span { class: "modal-sub mono", "{sub}" }
                        }
                    }
                    button {
                        class: "modal-x",
                        title: "Close",
                        onclick: move |_| onclose.call(()),
                        IconSvg { name: IconName::Close, size: 16 }
                    }
                }
                div { class: "modal-body", {children} }
            }
        }
    }
}

#[component]
pub fn Confirm(
    title: String,
    message: String,
    #[props(default = "Confirm".to_string())] confirm_label: String,
    #[props(default)] danger: bool,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Modal { title, onclose: move |_| oncancel.call(()),
            p { class: "confirm-message", "{message}" }
            div { class: "modal-actions",
                button {
                    class: "btn",
                    onclick: move |_| oncancel.call(()),
                    "CANCEL"
                }
                button {
                    class: if danger { "btn btn-danger" } else { "btn btn-accent" },
                    onclick: move |_| onconfirm.call(()),
                    "{confirm_label}"
                }
            }
        }
    }
}