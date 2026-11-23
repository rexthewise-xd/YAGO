use dioxus::prelude::*;
use storage::models::InstallStatus;

use crate::art::cover_url;
use crate::components::{status_dot, status_label, Modal};
use crate::icons::{IconName, IconSvg};
use crate::picker::pick_folder;
use crate::state::{Store, ToastKind};
use crate::views::GameSettingsDrawer;

#[component]
pub fn Overview() -> Element {
    let store = use_context::<Store>();
    let Some(game) = store.selected_game() else {
        return rsx! {
            div { class: "overview-empty",
                IconSvg { name: IconName::Grid, size: 32 }
                h2 { "No title selected" }
                p { "Add a game from the rail to get started." }
            }
        };
    };

    let cover = cover_url(&store, &game);
    let db = store.db(&game.id);
    let mod_count = db.as_ref().map(|d| d.mods.len()).unwrap_or(0);
    let profiles: Vec<_> = db
        .as_ref()
        .map(|d| {
            let mut p: Vec<_> = d.profiles.values().cloned().collect();
            p.sort_by(|a, b| a.added_at.cmp(&b.added_at));
            p
        })
        .unwrap_or_default();
    let active_profile = profiles
        .iter()
        .find(|p| p.id.to_string() == game.active_profile_id)
        .cloned();
    let enabled_count = active_profile
        .as_ref()
        .map(|p| p.enabled_mod_ids.len())
        .unwrap_or(0);

    let status = game.install_status;
    eprintln!(
        "DIAG overview status={:?} name={} desc_len={}",
        status,
        game.name,
        game.description.len()
    );
    let running = (store.running)();
    let launching = (store.launching)();
    let gid = game.id.clone();
    let gid2 = game.id.clone();
    let wish_gid = game.id.clone();
    let exe_path = game.exe_path.to_string_lossy().to_string();
    let install_path = game.install_path.to_string_lossy().to_string();

    let kill_store = store.clone();
    let mut launch_store = store.clone();
    let mut repair_store = store.clone();
    let mut install_store = store.clone();
    let trust_store = store.clone();
    let profile_store = store.clone();
    let repair_name = game.name.clone();
    let profile_gid = game.id.clone();
    let install_gid = game.id.clone();
    let install_name = game.name.clone();
    let mut show_game_settings = use_signal(|| false);
    let mut show_wish_url = use_signal(|| None::<String>);
    let mut extracting_wish = use_signal(|| false);
    let settings_gid = game.id.clone();

    let playtime_str = format_playtime(game.total_playtime_secs);
    let last_played_str = game
        .last_played
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Never".to_string());
    let is_exe_missing = status == InstallStatus::Installed && !game.exe_path.exists();

    rsx! {
        div { class: "overview",
            if is_exe_missing {
                div {
                    style: "background: rgba(239, 68, 68, 0.15); border: 1px solid rgba(239, 68, 68, 0.4); border-radius: 4px; padding: 12px 18px; margin: 16px 24px 0; display: flex; align-items: center; justify-content: space-between; gap: 16px;",
                    div { style: "display: flex; align-items: center; gap: 12px; color: #fca5a5;",
                        IconSvg { name: IconName::Warning, size: 24 }
                        div {
                            div { style: "font-weight: 700; font-size: 13px;", "Game Files Missing" }
                            div { class: "mono", style: "font-size: 11px; opacity: 0.85; margin-top: 2px;",
                                "Executable not found at {game.exe_path.display()}. It may have been moved or uninstalled."
                            }
                        }
                    }
                    button {
                        class: "btn btn-accent",
                        onclick: {
                            let store = store.clone();
                            let gid = game.id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    if let Some(folder) = pick_folder("Select Game Installation Folder").await {
                                        match core.relocate_game(gid, folder.display().to_string()).await {
                                            Ok(_) => s.toast(ToastKind::Success, "Game relocated successfully"),
                                            Err(e) => s.toast(ToastKind::Error, e),
                                        }
                                    }
                                });
                            }
                        },
                        IconSvg { name: IconName::Folder, size: 14 }
                        "RELOCATE FOLDER"
                    }
                }
            }

            div { class: "hero",
                img { class: "hero-art", src: "{cover}", alt: "" }
                div { class: "hero-shade" }
                div { class: "hero-content",
                    div { class: "hero-status mono",
                        span { class: status_dot(status) }
                        "{status_label(status)}"
                        if !game.version.is_empty() {
                            span { class: "hero-version", "v{game.version}" }
                        }
                        span { class: "hero-version", "PLAYTIME: {playtime_str}" }
                        span { class: "hero-version", "LAST: {last_played_str}" }
                    }
                    h1 { class: "hero-title", "{game.name}" }
                    p { class: "hero-desc", "{game.description}" }

                    div { class: "hero-actions",
                        if status == InstallStatus::Installed {
                            if running {
                                button {
                                    class: "btn btn-danger",
                                    onclick: move |_| {
                                        let core = kill_store.core.clone();
                                        kill_store.call(
                                            async move { core.kill_game().await },
                                            |s, _| s.toast(ToastKind::Success, "Game stopped"),
                                        );
                                    },
                                    IconSvg { name: IconName::Kill, size: 16 }
                                    "STOP"
                                }
                            } else {
                                button {
                                    class: "btn btn-accent",
                                    disabled: launching,
                                    onclick: move |_| launch_flow(&mut launch_store, gid.clone(), exe_path.clone()),
                                    IconSvg { name: IconName::Launch, size: 16 }
                                    if launching { "LAUNCHING…" } else { "LAUNCH" }
                                }
                            }
                            button {
                                class: "btn",
                                title: "Repair game files",
                                onclick: move |_| {
                                    let core = repair_store.core.clone();
                                    let gid = gid2.clone();
                                    let task = repair_store.task_start("Repair", repair_name.clone());
                                    repair_store.call(
                                        async move { core.repair_game(gid, Some(true)).await },
                                        move |s, _| s.task_finish(task, true, "Repair started"),
                                    );
                                },
                                IconSvg { name: IconName::Refresh, size: 15 }
                                "REPAIR"
                            }
                            button {
                                class: "btn",
                                disabled: (extracting_wish)(),
                                title: "Extract in-game Wish / Warp / Convene URL from web cache",
                                onclick: {
                                    let store = store.clone();
                                    let gid = wish_gid.clone();
                                    move |_| {
                                        let core = store.core.clone();
                                        let gid = gid.clone();
                                        let mut s = store.clone();
                                        spawn(async move {
                                            extracting_wish.set(true);
                                            match core.extract_wish_url(gid).await {
                                                Ok(url) => {
                                                    let _ = document::eval(&format!("navigator.clipboard.writeText({:?})", url));
                                                    show_wish_url.set(Some(url));
                                                    s.toast(ToastKind::Success, "Gacha URL extracted & copied to clipboard");
                                                }
                                                Err(e) => s.toast(ToastKind::Error, e),
                                            }
                                            extracting_wish.set(false);
                                        });
                                    }
                                },
                                IconSvg { name: IconName::Copy, size: 15 }
                                if (extracting_wish)() { "EXTRACTING…" } else { "WISH URL" }
                            }
                        } else if matches!(
                            status,
                            InstallStatus::Downloading | InstallStatus::Updating | InstallStatus::Queued
                        ) {
                            DownloadControls { game_id: game.id.clone() }
                        } else if status == InstallStatus::Remote {
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| {
                                    install_store.overlay.set(Some(crate::state::Overlay::Install {
                                        game_id: install_gid.clone(),
                                        name: install_name.clone(),
                                    }));
                                },
                                IconSvg { name: IconName::Download, size: 16 }
                                "INSTALL"
                            }
                        } else if status == InstallStatus::Corrupted {
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| {
                                    let core = trust_store.core.clone();
                                    let gid = gid2.clone();
                                    trust_store.call(
                                        async move { core.trust_game_installation(gid).await },
                                        |s, _| s.toast(ToastKind::Success, "Installation trusted"),
                                    );
                                },
                                IconSvg { name: IconName::Check, size: 16 }
                                "TRUST INSTALL"
                            }
                        }

                        button {
                            class: "icon-btn",
                            title: "Game Settings",
                            onclick: move |_| show_game_settings.set(true),
                            IconSvg { name: IconName::Settings, size: 18 }
                        }

                        {
                            let mut cover_store = store.clone();
                            let cover_gid = game.id.clone();
                            rsx! {
                                button {
                                    class: "icon-btn",
                                    title: "Change Cover",
                                    onclick: move |_| {
                                        cover_store.overlay.set(Some(crate::state::Overlay::CoverManager {
                                            game_id: cover_gid.clone(),
                                        }));
                                    },
                                    IconSvg { name: IconName::Image, size: 18 }
                                }
                            }
                        }
                    }
                }
            }

            div { class: "overview-grid",
                div { class: "panel",
                    div { class: "panel-heading", "LOADOUT" }
                    if profiles.is_empty() {
                        p { class: "muted", "No profiles yet." }
                    } else {
                        select {
                            class: "select",
                            value: "{game.active_profile_id}",
                            onchange: move |evt| {
                                let core = profile_store.core.clone();
                                let gid = profile_gid.clone();
                                let pid = evt.value();
                                profile_store.call(
                                    async move { core.switch_profile(gid, pid).await },
                                    |s, _| s.toast(ToastKind::Success, "Loadout switched"),
                                );
                            },
                            for p in profiles.iter() {
                                option {
                                    key: "{p.id}",
                                    value: "{p.id}",
                                    selected: p.id.to_string() == game.active_profile_id,
                                    "{p.name}"
                                }
                            }
                        }
                    }
                    div { class: "stat-row mono",
                        span { "{enabled_count} MODS ENABLED" }
                        span { "{mod_count} IN LIBRARY" }
                    }
                }

                div { class: "panel",
                    div { class: "panel-heading-row",
                        div { class: "panel-heading", "INSTALLATION" }
                        if !install_path.is_empty() {
                            button {
                                class: "icon-btn",
                                title: "Open in File Manager",
                                onclick: {
                                    let open_store = store.clone();
                                    let p = install_path.clone();
                                    move |_| {
                                        let core = open_store.core.clone();
                                        let p = p.clone();
                                        open_store.call(async move { core.open_path(p).await }, |_, _| {});
                                    }
                                },
                                IconSvg { name: IconName::Folder, size: 14 }
                            }
                        }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "PATH" }
                        span { class: "kv-val", title: "{install_path}", "{install_path}" }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "EXE" }
                        span { class: "kv-val", "{game.exe_name}" }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "SIZE" }
                        span { class: "kv-val", "{game.size}" }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "RUNNER" }
                        span { class: "kv-val",
                            {game.active_runner_id.clone().unwrap_or_else(|| "default".to_string())}
                        }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "PLAYTIME" }
                        span { class: "kv-val mono", "{playtime_str}" }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "LAST PLAYED" }
                        span { class: "kv-val mono", "{last_played_str}" }
                    }
                }

                div { class: "panel",
                    div { class: "panel-heading", "DEPLOYMENT" }
                    div { class: "kv",
                        span { class: "kv-key mono", "MODLOADER" }
                        span { class: "kv-val", if game.modloader_enabled { "ENABLED" } else { "DISABLED" } }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "SHIELD" }
                        span { class: "kv-val", if game.enable_linux_shield { "ACTIVE" } else { "OFF" } }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "INJECTION" }
                        span { class: "kv-val", "{game.injection_method:?}" }
                    }
                }
            }

            if (show_game_settings)() {
                GameSettingsDrawer {
                    game_id: settings_gid.clone(),
                    onclose: move |_| show_game_settings.set(false),
                }
            }

            if let Some(url) = (show_wish_url)() {
                Modal {
                    title: "GACHA HISTORY URL",
                    subtitle: Some("WISH / WARP / CONVENE LINK".to_string()),
                    onclose: move |_| show_wish_url.set(None),

                    p { class: "wizard-note", "This link contains your temporary in-game gacha authentication token. Copied to clipboard! You can paste it into tracker tools." }

                    div { class: "field-group",
                        textarea {
                            class: "input textarea mono",
                            readonly: true,
                            rows: 4,
                            value: "{url}",
                        }
                    }

                    div { class: "modal-actions",
                        button {
                            class: "btn btn-accent",
                            onclick: move |_| show_wish_url.set(None),
                            "DONE"
                        }
                    }
                }
            }
        }
    }
}

fn format_playtime(secs: u64) -> String {
    if secs == 0 {
        "0m".to_string()
    } else {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }
}

fn launch_flow(store: &mut Store, game_id: String, exe_path: String) {
    let core = store.core.clone();
    let close_on_launch = (store.settings)().close_on_launch;
    store.launching.set(true);
    let task = store.task_start("Launch", game_id.clone());
    let mut s = store.clone();
    spawn(async move {
        let result = async {
            core.ensure_game_resources(game_id.clone()).await?;
            let report = core.deploy_mods(exe_path.clone(), false).await?;
            if !report.overwritten_hashes.is_empty() {
                s.toast(
                    ToastKind::Info,
                    format!(
                        "{} mod hash collision(s) resolved via load order",
                        report.overwritten_hashes.len()
                    ),
                );
            }
            core.launch_game(game_id.clone()).await
        }
        .await;
        match result {
            Ok(()) => {
                s.launching.set(false);
                s.task_finish(task, true, "Game process started");
                if close_on_launch {
                    s.close_requested.set(true);
                }
            }
            Err(e) => {
                s.launching.set(false);
                s.task_finish(task, false, e.clone());
                s.toast(ToastKind::Error, e);
            }
        }
    });
}

#[component]
fn DownloadControls(game_id: String) -> Element {
    let store = use_context::<Store>();
    let downloads = (store.downloads)();
    let progress = downloads.get(&game_id).cloned();

    let pause_store = store.clone();
    let pause_id = game_id.clone();
    let resume_store = store.clone();
    let resume_id = game_id.clone();

    rsx! {
        div { class: "hero-download",
            if let Some(p) = progress {
                div { class: "progress-track hero-progress",
                    div { class: "progress-fill", style: "width: {p.percentage.clamp(0.0, 100.0)}%" }
                }
                div { class: "download-meta mono",
                    span { "{p.percentage:.1}%" }
                    span { "{crate::components::format_speed(p.speed_bps)}" }
                    span { "ETA {crate::components::format_eta(p.eta_secs)}" }
                }
            } else {
                div { class: "progress-track hero-progress",
                    div { class: "progress-fill indeterminate" }
                }
            }
            div { class: "hero-actions",
                button {
                    class: "btn",
                    title: "Pause download",
                    onclick: move |_| {
                        let core = pause_store.core.clone();
                        let gid = pause_id.clone();
                        pause_store.call(
                            async move { core.pause_game_download(gid).await },
                            |_, _| {},
                        );
                    },
                    IconSvg { name: IconName::Pause, size: 15 }
                    "PAUSE"
                }
                button {
                    class: "btn",
                    title: "Resume download",
                    onclick: move |_| {
                        let core = resume_store.core.clone();
                        let gid = resume_id.clone();
                        resume_store.call(
                            async move { core.resume_game_download(gid).await },
                            |_, _| {},
                        );
                    },
                    IconSvg { name: IconName::Launch, size: 15 }
                    "RESUME"
                }
            }
        }
    }
}
