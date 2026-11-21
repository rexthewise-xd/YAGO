use dioxus::prelude::*;
use storage::models::InstallStatus;

use crate::art::icon_url;
use crate::icons::{IconName, IconSvg};
use crate::state::Store;

#[component]
pub fn Rail() -> Element {
    let mut store = use_context::<Store>();
    let ids = store.game_ids();
    let selected = (store.selected)();

    rsx! {
        aside { class: "rail",
            div { class: "rail-heading", "LIBRARY" }

            for id in ids.iter() {
                RailTile { key: "{id}", id: id.clone(), selected: selected.as_deref() == Some(id.as_str()) }
            }

            button {
                class: "add-tile",
                title: "Add game",
                onclick: move |_| {
                    store.overlay.set(Some(crate::state::Overlay::AddGame));
                },
                IconSvg { name: IconName::Add, size: 18 }
            }

            div { class: "rail-spacer" }
            div { class: "rail-foot",
                "{ids.len()} TITLES"
            }
        }
    }
}

#[component]
fn RailTile(id: String, selected: bool) -> Element {
    let mut store = use_context::<Store>();
    let game = store.game(&id);
    let Some(game) = game else {
        return rsx! {};
    };

    let icon = icon_url(&store, &game);
    let accent = game.accent_color.clone();
    let dot = status_dot(game.install_status);
    let label = if game.short_name.is_empty() {
        game.logo_initial.clone()
    } else {
        game.short_name.clone()
    };
    let gid = id.clone();

    rsx! {
        button {
            class: if selected { "game-tile selected" } else { "game-tile" },
            style: "--tile-accent: {accent}",
            title: "{game.name}",
            onclick: move |_| {
                store.selected.set(Some(gid.clone()));
                store.view.set(crate::state::View::Overview);
            },
            div { class: "tile-icon",
                img { src: "{icon}", alt: "" }
                span { class: dot }
            }
            span { class: "tile-label", "{label}" }
        }
    }
}

pub fn status_dot(status: InstallStatus) -> &'static str {
    match status {
        InstallStatus::Installed => "status-dot ok",
        InstallStatus::Downloading | InstallStatus::Updating | InstallStatus::Queued => {
            "status-dot busy"
        }
        InstallStatus::Corrupted => "status-dot bad",
        InstallStatus::Remote => "status-dot missing",
    }
}

pub fn status_label(status: InstallStatus) -> &'static str {
    match status {
        InstallStatus::Installed => "READY",
        InstallStatus::Downloading => "DOWNLOADING",
        InstallStatus::Updating => "UPDATING",
        InstallStatus::Queued => "QUEUED",
        InstallStatus::Corrupted => "CORRUPTED",
        InstallStatus::Remote => "NOT INSTALLED",
    }
}