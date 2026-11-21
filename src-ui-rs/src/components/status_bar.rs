use dioxus::prelude::*;

use crate::state::Store;

#[component]
pub fn StatusBar() -> Element {
    let store = use_context::<Store>();
    let settings = (store.settings)();
    let game = store.selected_game();

    let runner = game
        .as_ref()
        .and_then(|g| g.active_runner_id.clone())
        .or_else(|| settings.default_runner_id.clone())
        .unwrap_or_else(|| "wine (system)".to_string());
    let prefix = game
        .as_ref()
        .and_then(|g| g.prefix_path.clone())
        .map(|p| p.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if settings.prefixes_path.as_os_str().is_empty() {
                "default prefix".to_string()
            } else {
                settings.prefixes_path.to_string_lossy().to_string()
            }
        });
    let storage = if settings.yago_storage_path.as_os_str().is_empty() {
        store.core.app_data_dir.to_string_lossy().to_string()
    } else {
        settings.yago_storage_path.to_string_lossy().to_string()
    };
    let running = (store.running)();

    rsx! {
        footer { class: "statusbar mono",
            span { class: if running { "sb-item sb-running" } else { "sb-item" },
                if running { "● GAME RUNNING" } else { "○ IDLE" }
            }
            span { class: "sb-item", "RUNNER {runner}" }
            span { class: "sb-item sb-ellipsis", title: "{prefix}", "PREFIX {prefix}" }
            span { class: "sb-spacer" }
            span { class: "sb-item sb-ellipsis", title: "{storage}", "STORAGE {storage}" }
        }
    }
}