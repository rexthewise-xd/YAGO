use dioxus::desktop::use_window;
use dioxus::prelude::*;

use crate::icons::{IconName, IconSvg};
use crate::state::{Store, View};

#[component]
pub fn TopStrip() -> Element {
    let mut store = use_context::<Store>();
    let window = use_window();
    let minimize = window.clone();
    let maximize = window.clone();
    let close = window.clone();

    let game = store.selected_game();
    let game_label = game
        .as_ref()
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "No title selected".to_string());
    let task_count = (store.tasks)()
        .iter()
        .filter(|t| t.state == crate::state::TaskState::Running)
        .count()
        + (store.downloads)().len();
    let stream_safe = (store.settings)().stream_safe;
    let mut shield_store = store.clone();

    rsx! {
        header {
            class: "topstrip",
            onmousedown: move |_| window.drag(),

            div { class: "brand",
                span { class: "brand-mark" }
                span { class: "brand-name", "YAGO" }
                span { class: "brand-sep", "▸" }
                span { class: "brand-game", "{game_label}" }
            }

            nav { class: "tabs",
                onmousedown: move |evt| evt.stop_propagation(),
                for view in [View::Overview, View::Mods, View::Loadouts, View::Skins] {
                    button {
                        class: if (store.view)() == view { "tab active" } else { "tab" },
                        onclick: move |_| store.view.set(view),
                        IconSvg { name: tab_icon(view), size: 14 }
                        span { class: "tab-label", "{view.title()}" }
                    }
                }
            }

            div { class: "strip-right",
                onmousedown: move |evt| evt.stop_propagation(),

                if task_count > 0 {
                    div { class: "task-count", title: "Active tasks",
                        span { class: "pulse-dot" }
                        "{task_count} ACTIVE"
                    }
                }

                button {
                    class: if stream_safe { "strip-btn shield-on" } else { "strip-btn" },
                    title: "Stream-safe mode",
                    onclick: move |_| {
                        let mut settings = (shield_store.settings)();
                        settings.stream_safe = !settings.stream_safe;
                        let core = shield_store.core.clone();
                        shield_store.settings.set(settings.clone());
                        shield_store.call(
                            async move { core.update_settings(settings).await },
                            |_, _| {},
                        );
                    },
                    IconSvg { name: IconName::Shield, size: 15 }
                }

                button {
                    class: if (store.view)() == View::Settings { "strip-btn active" } else { "strip-btn" },
                    title: "Settings",
                    onclick: move |_| store.view.set(View::Settings),
                    IconSvg { name: IconName::Settings, size: 15 }
                }

                div { class: "win-controls",
                    button {
                        class: "win-btn",
                        title: "Minimize",
                        onclick: move |_| minimize.set_minimized(true),
                        IconSvg { name: IconName::Minimize, size: 14 }
                    }
                    button {
                        class: "win-btn",
                        title: "Maximize",
                        onclick: move |_| maximize.toggle_maximized(),
                        IconSvg { name: IconName::Maximize, size: 12 }
                    }
                    button {
                        class: "win-btn danger",
                        title: "Close",
                        onclick: move |_| close.close(),
                        IconSvg { name: IconName::Close, size: 14 }
                    }
                }
            }
        }
    }
}

fn tab_icon(view: View) -> IconName {
    match view {
        View::Overview => IconName::Home,
        View::Mods => IconName::Mods,
        View::Loadouts => IconName::Profiles,
        View::Skins => IconName::Skins,
        View::Settings => IconName::Settings,
    }
}