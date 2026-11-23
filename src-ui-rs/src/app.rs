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
    let init_result = use_context::<Result<AppCore, String>>();
    let mut init_state = use_signal(move || init_result);
    let mut retrying = use_signal(|| false);

    match (init_state)() {
        Err(err) => rsx! {
            document::Stylesheet { href: THEME_CSS }
            document::Stylesheet { href: THEMES_CSS }

            div {
                class: "app theme-mission",
                style: "height: 100vh; display: flex; align-items: center; justify-content: center; background: #0b0e14; padding: 24px;",
                div {
                    style: "max-width: 520px; width: 100%; background: #12161f; border: 1px solid #f87171; border-radius: 6px; padding: 28px; display: flex; flex-direction: column; align-items: center; gap: 16px; text-align: center; box-shadow: 0 16px 40px rgba(0,0,0,0.6);",
                    div {
                        style: "color: #f87171;",
                        IconSvg { name: IconName::Warning, size: 48 }
                    }
                    div {
                        h2 { class: "mono", style: "font-size: 16px; color: #f87171; letter-spacing: 2px;", "STARTUP INITIALIZATION FAILED" }
                        p { class: "mono", style: "font-size: 11px; color: #8b94a7; margin-top: 4px;", "YAGO encountered an error preparing core libraries and storage." }
                    }
                    div {
                        style: "width: 100%; max-height: 160px; overflow-y: auto; background: #0b0e14; border: 1px solid #2a3242; border-radius: 4px; padding: 12px; font-family: monospace; font-size: 11px; color: #fca5a5; text-align: left; word-break: break-all;",
                        "{err}"
                    }
                    div { style: "display: flex; gap: 10px; align-items: center; margin-top: 8px;",
                        button {
                            class: "btn btn-accent",
                            disabled: (retrying)(),
                            onclick: move |_| {
                                retrying.set(true);
                                spawn(async move {
                                    let res = yago_app::init().await.map_err(|e| e.to_string());
                                    init_state.set(res);
                                    retrying.set(false);
                                });
                            },
                            IconSvg { name: IconName::Refresh, size: 14 }
                            if (retrying)() { "RETRYING…" } else { "RETRY STARTUP" }
                        }
                        button {
                            class: "btn",
                            onclick: move |_| {
                                #[cfg(target_os = "linux")]
                                let opener = "xdg-open";
                                #[cfg(target_os = "windows")]
                                let opener = "explorer";
                                #[cfg(target_os = "macos")]
                                let opener = "open";
                                let _ = std::process::Command::new(opener)
                                    .arg("https://github.com/rexthewise-xd/YAGO/issues/new")
                                    .spawn();
                            },
                            IconSvg { name: IconName::Launch, size: 14 }
                            "REPORT ON GITHUB"
                        }
                    }
                }
            }
        },
        Ok(core) => rsx! {
            AppShell { key: "{core.app_data_dir.display()}", core }
        },
    }
}

#[component]
fn AppShell(core: AppCore) -> Element {
    let store = use_context_provider(|| Store::new(core.clone()));

    // Initial background library and state refresh
    let refresh_store = store.clone();
    use_future(move || {
        let mut s = refresh_store.clone();
        async move {
            s.refresh().await;
        }
    });

    // Event bus listener
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

    // Emergency Panic shortcut
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

    // Command Palette shortcut
    let mut show_command_palette = use_signal(|| false);
    {
        let _ = use_global_shortcut("Ctrl+K", move |_| {
            let curr = (show_command_palette)();
            show_command_palette.set(!curr);
        });
    }

    // 1-Click install deep link handler (handles initial CLI args and live forwarded links)
    {
        let store = store.clone();
        let deep_link_ctx = try_consume_context::<crate::DeepLinkReceiver>();
        use_future(move || {
            let mut store = store.clone();
            let deep_link_ctx = deep_link_ctx.clone();
            async move {
                if let Some(ch) = deep_link_ctx {
                    let mut rx = ch.lock().await;
                    while let Some(uri) = rx.recv().await {
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
            }
        });
    }

    // Window close request monitor
    {
        let store = store.clone();
        let window = use_window();
        use_effect(move || {
            if (store.close_requested)() {
                window.close();
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
    let is_loaded = (store.loaded)();
    let is_setup_pending = (store.setup_done)() == Some(false);
    let mut crash_log_banner = use_signal(|| core.find_recent_crash());

    rsx! {
        document::Stylesheet { href: THEME_CSS }
        document::Stylesheet { href: THEMES_CSS }

        div {
            class: "app {theme_class}",
            style: "--game-accent: {accent}; background-color: var(--bg);",
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

            if let Some(crash_path) = (crash_log_banner)() {
                div {
                    style: "background: rgba(239, 68, 68, 0.15); border-bottom: 1px solid rgba(239, 68, 68, 0.4); padding: 6px 16px; display: flex; align-items: center; justify-content: space-between; font-size: 11px; z-index: 100;",
                    div { style: "display: flex; align-items: center; gap: 8px; color: #fca5a5;",
                        IconSvg { name: IconName::Warning, size: 14 }
                        span { class: "mono", "YAGO recovered from an unexpected crash in a previous session." }
                    }
                    div { style: "display: flex; gap: 8px; align-items: center;",
                        button {
                            class: "btn btn-sm",
                            onclick: {
                                let cp = crash_path.clone();
                                let store = store.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let p = cp.display().to_string();
                                    spawn(async move {
                                        let _ = core.open_path(p).await;
                                    });
                                }
                            },
                            "VIEW CRASH LOG"
                        }
                        button {
                            class: "btn btn-sm btn-accent",
                            onclick: {
                                let store = store.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let mut s = store.clone();
                                    spawn(async move {
                                        match core.export_bug_report(None).await {
                                            Ok(zip_path) => {
                                                s.toast(crate::state::ToastKind::Success, format!("Bug report created: {zip_path}"));
                                                let _ = core.open_path(zip_path).await;
                                            }
                                            Err(e) => s.toast(crate::state::ToastKind::Error, format!("Failed to create bug report: {e}")),
                                        }
                                    });
                                }
                            },
                            IconSvg { name: IconName::Download, size: 13 }
                            "CREATE BUG REPORT"
                        }
                        button {
                            class: "btn btn-sm",
                            onclick: {
                                let store = store.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    spawn(async move {
                                        let _ = core.open_path("https://github.com/rexthewise-xd/YAGO/issues/new".to_string()).await;
                                    });
                                }
                            },
                            IconSvg { name: IconName::Launch, size: 13 }
                            "REPORT ON GITHUB"
                        }
                        button {
                            class: "btn btn-sm",
                            onclick: {
                                let cp = crash_path.clone();
                                let store = store.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let _ = core.acknowledge_crash(&cp);
                                    crash_log_banner.set(None);
                                }
                            },
                            "DISMISS"
                        }
                    }
                }
            }

            TopStrip {}

            if !is_loaded {
                // Themed Initializing YAGO splash screen while loading
                div {
                    style: "flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px; background: var(--bg);",
                    div {
                        style: "width: 44px; height: 44px; background: var(--accent); border-radius: 6px; display: flex; align-items: center; justify-content: center; color: #0b0e14;",
                        IconSvg { name: IconName::Launch, size: 24 }
                    }
                    div { style: "text-align: center;",
                        h2 { class: "mono", style: "font-size: 14px; letter-spacing: 2px; color: var(--text);", "INITIALIZING YAGO…" }
                        p { class: "mono", style: "font-size: 11px; color: var(--muted); margin-top: 4px;", "Preparing game library and loaders" }
                    }
                    div { class: "progress-track", style: "width: 160px; height: 3px;",
                        div { class: "progress-fill indeterminate" }
                    }
                }
            } else if is_setup_pending {
                // First-time setup wizard fills main content cleanly
                div { style: "flex: 1; overflow-y: auto; background: var(--bg);",
                    SetupWizard {}
                }
            } else {
                div { class: "shell",
                    Rail {}
                    main { class: "content",
                        match (store.view)() {
                            View::Overview => rsx! { Overview {} },
                            View::Mods => rsx! { ModsView {} },
                            View::Loadouts => rsx! { LoadoutsView {} },
                            View::Skins => rsx! { SkinsView {} },
                            View::Settings => rsx! { SettingsView {} },
                        }
                    }
                    ActivityPanel {}
                }
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
