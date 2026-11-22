use dioxus::desktop::{use_global_shortcut, use_window};
use dioxus::prelude::*;
use tokio::sync::broadcast::error::RecvError;
use yago_app::AppCore;

use crate::components::{ActivityPanel, CommandPalette, Rail, StatusBar, Toasts, TopStrip};
use crate::icons::{IconName, IconSvg};
use crate::state::{Overlay, Store, View};
use crate::views::{
    AddGameModal, CoverManagerModal, InstallWizard, LoadoutsView, ModsView, Overview, SettingsView,
    SetupWizard, SkinsView,
};

static THEME_CSS: Asset = asset!("/assets/theme.css");
static THEMES_CSS: Asset = asset!("/assets/themes.css");

#[component]
pub fn App() -> Element {
    let core = use_context::<AppCore>();
    let store = use_context_provider(|| Store::new(core.clone()));

    {
        let store = store.clone();
        use_resource(move || {
            let mut store = store.clone();
            async move {
                store.refresh().await;
            }
        });
    }

    {
        let store = store.clone();
        let mut rx = Some(store.core.events.subscribe());
        use_future(move || {
            let mut rx = rx.take().unwrap();
            let mut store = store.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            store.apply_event(event);
                            while let Ok(event) = rx.try_recv() {
                                store.apply_event(event);
                            }
                        }
                        Err(RecvError::Lagged(_)) => {
                            store.refresh().await;
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            }
        });
    }

    {
        let store = store.clone();
        let _ = use_global_shortcut("Alt+F12", move |_| {
            let mut store = store.clone();
            store.panic.set(true);
            let core = store.core.clone();
            spawn(async move {
                let _ = core.trigger_panic().await;
            });
        });
    }

    let mut show_command_palette = use_signal(|| false);

    {
        let _ = use_global_shortcut("Ctrl+K", move |_| {
            let curr = (show_command_palette)();
            show_command_palette.set(!curr);
        });
    }

    {
        let store = store.clone();
        use_future(move || {
            let mut store = store.clone();
            async move {
                let deep_link = std::env::args().find(|a| a.starts_with("yago://"));
                if let Some(uri) = deep_link {
                    let mut attempts = 0;
                    while store.selected_game().is_none() && attempts < 30 {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        attempts += 1;
                    }
                    if let Some(game) = store.selected_game() {
                        let core = store.core.clone();
                        store.toast(
                            crate::state::ToastKind::Info,
                            "Processing 1-Click mod install deep link...",
                        );
                        match core.handle_deep_link(game.id.clone(), uri).await {
                            Ok(rec) => {
                                store.toast(
                                    crate::state::ToastKind::Success,
                                    format!("Installed {} via 1-Click", rec.meta.name),
                                );
                            }
                            Err(e) => {
                                store.toast(
                                    crate::state::ToastKind::Error,
                                    format!("1-Click install failed: {e}"),
                                );
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let store = store.clone();
        let window = use_window();
        use_resource(move || {
            let close = (store.close_requested)();
            let window = window.clone();
            async move {
                if close {
                    window.close();
                }
            }
        });
    }

    let theme = (store.app_config)().theme;
    let theme_class = if crate::themes::THEME_IDS.contains(&theme.as_str()) {
        format!("theme-{}", theme)
    } else {
        "theme-mission".to_string()
    };

    let selected = store.selected_game();
    let accent = selected
        .as_ref()
        .map(|g| g.accent_color.clone())
        .unwrap_or_else(|| "#22d3ee".to_string());

    let mut key_store = store.clone();
    let mut dismiss_store = store.clone();

    rsx! {
        document::Stylesheet { href: THEME_CSS }
        document::Stylesheet { href: THEMES_CSS }

        div {
            class: "app {theme_class}",
            style: "--game-accent: {accent}",
            tabindex: "0",
            onkeydown: move |evt| {
                if evt.key() == Key::F12 && evt.modifiers().alt() {
                    key_store.panic.set(true);
                    let core = key_store.core.clone();
                    spawn(async move {
                        let _ = core.trigger_panic().await;
                    });
                }
                let key_dbg = format!("{:?}", evt.key());
                if (evt.modifiers().ctrl() || evt.modifiers().meta())
                    && (key_dbg.contains('k') || key_dbg.contains('K'))
                {
                    let curr = (show_command_palette)();
                    show_command_palette.set(!curr);
                }
            },

            TopStrip {}

            div { class: "shell",
                Rail {}
                main { class: "content",
                    if !(store.loaded)() {
                        div { class: "loading", "INITIALIZING LIBRARY…" }
                    } else if (store.setup_done)() == Some(false) {
                        SetupWizard {}
                    } else {
                        match (store.view)() {
                            View::Overview => rsx! { Overview {} },
                            View::Mods => rsx! { ModsView {} },
                            View::Loadouts => rsx! { LoadoutsView {} },
                            View::Skins => rsx! { SkinsView {} },
                            View::Settings => rsx! { SettingsView {} },
                        }
                    }
                }
                ActivityPanel {}
            }

            StatusBar {}
            Toasts {}

            match (store.overlay)() {
                Some(Overlay::AddGame) => rsx! { AddGameModal {} },
                Some(Overlay::Install { game_id, name }) => rsx! {
                    InstallWizard { key: "{game_id}", game_id, name }
                },
                Some(Overlay::CoverManager { game_id }) => {
                    let mut s = store.clone();
                    rsx! {
                        CoverManagerModal {
                            key: "{game_id}",
                            game_id,
                            onclose: move |_| { s.overlay.set(None); },
                        }
                    }
                }
                None => rsx! {},
            }

            if (store.panic)() {
                div { class: "panic-overlay",
                    div { class: "panic-box",
                        IconSvg { name: IconName::Shield, size: 42 }
                        h2 { "PANIC TRIGGERED" }
                        p { "Deployments purged. Modloader reloaded." }
                        button {
                            class: "btn",
                            onclick: move |_| {
                                dismiss_store.panic.set(false);
                            },
                            "Dismiss"
                        }
                    }
                }
            }

            CommandPalette { is_open: show_command_palette }
        }
    }
}
