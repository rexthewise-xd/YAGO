use dioxus::prelude::*;
use yago_app::{
    GameConfigUpdate, GraphicsAuditWarning, InjectionMethod, ProfileUpdate, StartMethod,
    TexFxConfig, VoicePackStatus,
};

use crate::icons::{IconName, IconSvg};
use crate::picker::pick_folder;
use crate::state::{Store, ToastKind};

#[derive(Clone, Copy, PartialEq)]
enum GTab {
    General,
    Installation,
    Advanced,
    Management,
}

#[component]
pub fn GameSettingsDrawer(game_id: String, onclose: EventHandler<()>) -> Element {
    let store = use_context::<Store>();
    let mut active_tab = use_signal(|| GTab::General);
    let tab = (active_tab)();

    let db = store.db(&game_id);
    let game = db.as_ref().and_then(|db| db.games.get(&game_id).cloned());
    let Some(game) = game else {
        return rsx! {
            div { class: "drawer-backdrop", onclick: move |_| onclose.call(()),
                div { class: "drawer", "Game not found" }
            }
        };
    };

    let game_name = game.name.clone();

    rsx! {
        div { class: "drawer-backdrop", onclick: move |_| onclose.call(()),
            div { class: "drawer", onclick: move |e| e.stop_propagation(),
                div { class: "drawer-header",
                    div {
                        h2 { "GAME SETTINGS" }
                        span { class: "drawer-subtitle mono", "{game_name}" }
                    }
                    button {
                        class: "icon-btn",
                        onclick: move |_| onclose.call(()),
                        IconSvg { name: IconName::Close, size: 18 }
                    }
                }

                div { class: "settings-tabs",
                    for &(t, label) in &[(GTab::General, "GENERAL"), (GTab::Installation, "INSTALL"), (GTab::Advanced, "ADVANCED"), (GTab::Management, "MANAGE")] {
                        button {
                            class: if tab == t { "seg-btn active" } else { "seg-btn" },
                            onclick: move |_| active_tab.set(t),
                            "{label}"
                        }
                    }
                }

                div { class: "drawer-body",
                    match tab {
                        GTab::General => rsx! {
                            GameGeneralTab { game_id: game_id.clone() }
                        },
                        GTab::Installation => rsx! {
                            GameInstallTab { game_id: game_id.clone() }
                        },
                        GTab::Advanced => rsx! {
                            GameAdvancedTab { game_id: game_id.clone() }
                        },
                        GTab::Management => rsx! {
                            GameManagementTab { game_id: game_id.clone() }
                        },
                    }
                }
            }
        }
    }
}

// ─── General ─────────────────────────────────────────────────────

#[component]
fn GameGeneralTab(game_id: String) -> Element {
    let store = use_context::<Store>();
    let db = store.db(&game_id);
    let game = db.as_ref().and_then(|db| db.games.get(&game_id).cloned());
    let Some(game) = game else { return rsx! {} };

    let mut name = use_signal(|| game.name.clone());
    let mut developer = use_signal(|| game.developer.clone());
    let mut description = use_signal(|| game.description.clone());
    let mut color = use_signal(|| game.color.clone());
    let mut accent = use_signal(|| game.accent_color.clone());
    let mut logo_initial = use_signal(|| game.logo_initial.clone());

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "IDENTITY" }

            div { class: "field-group",
                label { class: "field-label", "Game Name" }
                input {
                    class: "input",
                    value: "{(name)()}",
                    oninput: move |e| name.set(e.value()),
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Developer" }
                input {
                    class: "input",
                    value: "{(developer)()}",
                    oninput: move |e| developer.set(e.value()),
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Description" }
                textarea {
                    class: "input textarea",
                    rows: 3,
                    value: "{(description)()}",
                    oninput: move |e| description.set(e.value()),
                }
            }

            div { class: "field-row",
                div { class: "field-group half",
                    label { class: "field-label", "Color" }
                    input {
                        class: "input mono",
                        value: "{(color)()}",
                        oninput: move |e| color.set(e.value()),
                    }
                }
                div { class: "field-group half",
                    label { class: "field-label", "Accent" }
                    input {
                        class: "input mono",
                        value: "{(accent)()}",
                        oninput: move |e| accent.set(e.value()),
                    }
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Logo Initial" }
                input {
                    class: "input mono",
                    value: "{(logo_initial)()}",
                    maxlength: 2,
                    oninput: move |e| logo_initial.set(e.value()),
                }
            }

            button {
                class: "btn btn-accent btn-block",
                onclick: {
                    let store = store.clone();
                    let gid = game_id.clone();
                    move |_| {
                        let core = store.core.clone();
                        let gid = gid.clone();
                        let update = GameConfigUpdate {
                            name: Some((name)()),
                            developer: Some((developer)()),
                            description: Some((description)()),
                            color: Some((color)()),
                            accent_color: Some((accent)()),
                            logo_initial: Some((logo_initial)()),
                            ..Default::default()
                        };
                        let s = store.clone();
                        s.call(
                            async move { core.update_game_config(gid, update).await },
                            |s, _| s.toast(ToastKind::Success, "Game info saved"),
                        );
                    }
                },
                IconSvg { name: IconName::Save, size: 14 }
                "SAVE CHANGES"
            }
        }
    }
}

// ─── Installation ────────────────────────────────────────────────

#[component]
fn GameInstallTab(game_id: String) -> Element {
    let store = use_context::<Store>();
    let db = store.db(&game_id);
    let game = db.as_ref().and_then(|db| db.games.get(&game_id).cloned());
    let Some(game) = game else { return rsx! {} };

    let mut install_path = use_signal(|| game.install_path.display().to_string());
    let mut exe_name = use_signal(|| game.exe_name.clone());
    let mut launch_args = use_signal(|| game.launch_args.join(" "));
    let mut prefix_path = use_signal(|| {
        game.prefix_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    });

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "GAME FILES" }

            div { class: "field-group",
                label { class: "field-label", "Install Path" }
                div { class: "path-editor-row",
                    input {
                        class: "input mono",
                        value: "{(install_path)()}",
                        oninput: move |e| install_path.set(e.value()),
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            spawn(async move {
                                if let Some(path) = pick_folder("Select Game Folder").await {
                                    install_path.set(path.display().to_string());
                                }
                            });
                        },
                        IconSvg { name: IconName::Folder, size: 16 }
                    }
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Executable Name" }
                input {
                    class: "input mono",
                    value: "{(exe_name)()}",
                    oninput: move |e| exe_name.set(e.value()),
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Launch Arguments" }
                input {
                    class: "input mono",
                    placeholder: "-arg1 -arg2 ...",
                    value: "{(launch_args)()}",
                    oninput: move |e| launch_args.set(e.value()),
                }
            }

            div { class: "field-group",
                label { class: "field-label", "Prefix Path (Wine/Proton)" }
                div { class: "path-editor-row",
                    input {
                        class: "input mono",
                        placeholder: "Uses default if empty",
                        value: "{(prefix_path)()}",
                        oninput: move |e| prefix_path.set(e.value()),
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            spawn(async move {
                                if let Some(path) = pick_folder("Select Prefix").await {
                                    prefix_path.set(path.display().to_string());
                                }
                            });
                        },
                        IconSvg { name: IconName::Folder, size: 16 }
                    }
                }
            }

            button {
                class: "btn btn-accent btn-block",
                onclick: {
                    let store = store.clone();
                    let gid = game_id.clone();
                    move |_| {
                        let core = store.core.clone();
                        let gid = gid.clone();
                        let la: Vec<String> = (launch_args)().split_whitespace().map(|s| s.to_string()).collect();
                        let pp = {
                            let p = (prefix_path)();
                            if p.is_empty() { None } else { Some(p) }
                        };
                        let update = GameConfigUpdate {
                            install_path: Some((install_path)()),
                            exe_name: Some((exe_name)()),
                            launch_args: Some(la),
                            prefix_path: Some(pp),
                            ..Default::default()
                        };
                        let s = store.clone();
                        s.call(
                            async move { core.update_game_config(gid, update).await },
                            |s, _| s.toast(ToastKind::Success, "Installation settings saved"),
                        );
                    }
                },
                IconSvg { name: IconName::Save, size: 14 }
                "SAVE"
            }
        }
    }
}

// ─── Advanced ────────────────────────────────────────────────────

#[component]
fn GameAdvancedTab(game_id: String) -> Element {
    let store = use_context::<Store>();
    let db = store.db(&game_id);
    let game = db.as_ref().and_then(|db| db.games.get(&game_id).cloned());
    let Some(game) = game else { return rsx! {} };

    let injection = game.injection_method;
    let modloader_on = game.modloader_enabled;
    let shield_on = game.enable_linux_shield;
    let auto_update_on = game.auto_update;
    let start_method = game.start_method;
    let unsafe_mode_on = game.unsafe_mode;
    let use_umu_on = game.use_umu;
    let mut umu_id = use_signal(|| game.umu_game_id.clone().unwrap_or_default());
    let mut pre_cmd = use_signal(|| game.pre_launch_cmd.clone().unwrap_or_default());
    let mut post_cmd = use_signal(|| game.post_exit_cmd.clone().unwrap_or_default());

    let mut audit_results = use_signal(|| None::<Vec<GraphicsAuditWarning>>);
    let mut auditing = use_signal(|| false);
    let mut applying_tweaks = use_signal(|| false);

    let hunting_mode_on = game.hunting_mode;
    let mut gui_collect_path = use_signal(|| {
        game.gui_collect_ini
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    });
    let mut log_diagnostics = use_signal(|| None::<Vec<yago_app::LogDiagnostic>>);
    let mut reading_log = use_signal(|| false);

    let profile = {
        let p_id = &game.active_profile_id;
        let db = store.db(&game_id);
        db.as_ref().and_then(|d| {
            uuid::Uuid::parse_str(p_id)
                .ok()
                .and_then(|u| d.profiles.get(&u).cloned())
        })
    };

    let use_gamemode = profile.as_ref().map(|p| p.use_gamemode).unwrap_or(false);
    let use_gamescope = profile.as_ref().map(|p| p.use_gamescope).unwrap_or(false);
    let use_mangohud = profile.as_ref().map(|p| p.use_mangohud).unwrap_or(false);
    let use_reshade = profile.as_ref().map(|p| p.use_reshade).unwrap_or(false);

    let runners_core = store.core.clone();
    let runner_list = use_resource(move || {
        let core = runners_core.clone();
        async move { core.list_runners().await.unwrap_or_default() }
    });
    let runners = (runner_list)().unwrap_or_default();
    let current_runner = game.active_runner_id.clone().unwrap_or_default();

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "MOD LOADER" }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Mods, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Mod Loader" }
                        div { class: "setting-desc", "Enable the mod injection framework" }
                    }
                }
                button {
                    class: if modloader_on { "toggle-switch on" } else { "toggle-switch" },
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                modloader_enabled: Some(!modloader_on),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    div { class: "toggle-knob" }
                }
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Refresh, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Automatic Updates" }
                        div { class: "setting-desc", "Automatically check and apply game delta updates" }
                    }
                }
                button {
                    class: if auto_update_on { "toggle-switch on" } else { "toggle-switch" },
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                auto_update: Some(!auto_update_on),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    div { class: "toggle-knob" }
                }
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Settings, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Injection Method" }
                        div { class: "setting-desc", "How mods are injected into the game" }
                    }
                }
                select {
                    class: "input select-sm",
                    value: "{injection:?}",
                    onchange: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |e: Event<FormData>| {
                            let method = match e.value().as_str() {
                                "Proxy" => InjectionMethod::Proxy,
                                "Loader" => InjectionMethod::Loader,
                                "RemoteThread" => InjectionMethod::RemoteThread,
                                "ManualMap" => InjectionMethod::ManualMap,
                                _ => InjectionMethod::None,
                            };
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                injection_method: Some(method),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    option { value: "None", "None" }
                    option { value: "Proxy", "DLL Proxy" }
                    option { value: "Loader", "Loader" }
                    option { value: "RemoteThread", "Remote Thread" }
                    option { value: "ManualMap", disabled: true, "Manual Map (Unsupported)" }
                }
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Settings, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Start Method" }
                        div { class: "setting-desc", "Process spawning strategy (Shell recommended for HoYo games on Windows)" }
                    }
                }
                select {
                    class: "input select-sm",
                    value: "{start_method:?}",
                    onchange: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |e: Event<FormData>| {
                            let method = match e.value().as_str() {
                                "Shell" => StartMethod::Shell,
                                "Steam" => StartMethod::Steam,
                                "Manual" => StartMethod::Manual,
                                _ => StartMethod::Native,
                            };
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                start_method: Some(method),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    option { value: "Native", "Native Process" }
                    option { value: "Shell", "Shell Execution (cmd /C start)" }
                    option { value: "Steam", "Steam Protocol" }
                    option { value: "Manual", "Manual Launch" }
                }
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Shield, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Unsafe Mode (Pin Custom DLLs)" }
                        div { class: "setting-desc", "Preserve manually placed d3d11.dll and d3dx.ini without overwriting" }
                    }
                }
                button {
                    class: if unsafe_mode_on { "toggle-switch on" } else { "toggle-switch" },
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                unsafe_mode: Some(!unsafe_mode_on),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    div { class: "toggle-knob" }
                }
            }
        }

        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "RUNNER" }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Settings, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Runner (Proton/Wine)" }
                        div { class: "setting-desc", "Override the system default runner for this game" }
                    }
                }
                select {
                    class: "input select-sm",
                    value: "{current_runner}",
                    onchange: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |e: Event<FormData>| {
                            let val = e.value();
                            let runner = if val.is_empty() { None } else { Some(val) };
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                active_runner_id: Some(runner),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    option { value: "", "System Default" }
                    for r in runners.iter() {
                        option { value: "{r}", "{r}" }
                    }
                }
            }
        }

        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "LINUX ENVIRONMENT" }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Shield, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Linux Shield" }
                        div { class: "setting-desc", "Block game telemetry and anti-cheat" }
                    }
                }
                button {
                    class: if shield_on { "toggle-switch on" } else { "toggle-switch" },
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                enable_linux_shield: Some(!shield_on),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    div { class: "toggle-knob" }
                }
            }

            ProfileToggle { label: "GameMode", desc: "CPU/GPU performance governor via gamemode", checked: use_gamemode, field: "use_gamemode", game_id: game_id.clone() }
            ProfileToggle { label: "Gamescope", desc: "Run game inside a Gamescope compositor", checked: use_gamescope, field: "use_gamescope", game_id: game_id.clone() }
            ProfileToggle { label: "MangoHud", desc: "In-game FPS / stats overlay", checked: use_mangohud, field: "use_mangohud", game_id: game_id.clone() }
            ProfileToggle { label: "ReShade", desc: "Post-processing shader framework", checked: use_reshade, field: "use_reshade", game_id: game_id.clone() }

            div { class: "setting-row setting-row-col",
                div { class: "setting-row-top",
                    div { class: "setting-row-left",
                        div { class: "setting-icon",
                            IconSvg { name: IconName::Deploy, size: 18 }
                        }
                        div {
                            div { class: "setting-label", "UMU Launcher (umu-run)" }
                            div { class: "setting-desc", "Route execution through Unified Linux Wine Game Launcher" }
                        }
                    }
                    button {
                        class: if use_umu_on { "toggle-switch on" } else { "toggle-switch" },
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let update = GameConfigUpdate {
                                    use_umu: Some(!use_umu_on),
                                    ..Default::default()
                                };
                                let s = store.clone();
                                s.call(
                                    async move { core.update_game_config(gid, update).await },
                                    |_, _| {},
                                );
                            }
                        },
                        div { class: "toggle-knob" }
                    }
                }
                if use_umu_on {
                    div { style: "display: flex; align-items: center; gap: 8px; margin-top: 8px;",
                        span { class: "mono", style: "font-size: 11px; color: var(--text-sec);", "GAMEID:" }
                        input {
                            class: "input",
                            style: "flex: 1; font-size: 11px;",
                            placeholder: "e.g. umu-genshin, umu-wuwa (default: 0)",
                            value: "{(umu_id)()}",
                            oninput: move |e| umu_id.set(e.value()),
                            onblur: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |_| {
                                    let val = (umu_id)().trim().to_string();
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let update = GameConfigUpdate {
                                        umu_game_id: Some(if val.is_empty() { None } else { Some(val) }),
                                        ..Default::default()
                                    };
                                    let s = store.clone();
                                    s.call(
                                        async move { core.update_game_config(gid, update).await },
                                        |_, _| {},
                                    );
                                }
                            },
                        }
                    }
                }
            }

            div { class: "setting-row setting-row-col",
                div { class: "setting-label", style: "margin-bottom: 6px;", "Custom Launch Commands" }
                div { style: "display: flex; flex-direction: column; gap: 6px;",
                    div { style: "display: flex; align-items: center; gap: 8px;",
                        span { class: "mono", style: "font-size: 10px; color: var(--text-sec); width: 80px;", "Pre-Launch:" }
                        input {
                            class: "input mono",
                            style: "flex: 1; font-size: 11px;",
                            placeholder: "Command to run before game starts...",
                            value: "{(pre_cmd)()}",
                            oninput: move |e| pre_cmd.set(e.value()),
                            onblur: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |_| {
                                    let val = (pre_cmd)().trim().to_string();
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let update = GameConfigUpdate {
                                        pre_launch_cmd: Some(if val.is_empty() { None } else { Some(val) }),
                                        ..Default::default()
                                    };
                                    let s = store.clone();
                                    s.call(
                                        async move { core.update_game_config(gid, update).await },
                                        |_, _| {},
                                    );
                                }
                            },
                        }
                    }
                    div { style: "display: flex; align-items: center; gap: 8px;",
                        span { class: "mono", style: "font-size: 10px; color: var(--text-sec); width: 80px;", "Post-Exit:" }
                        input {
                            class: "input mono",
                            style: "flex: 1; font-size: 11px;",
                            placeholder: "Command to run after game process exits...",
                            value: "{(post_cmd)()}",
                            oninput: move |e| post_cmd.set(e.value()),
                            onblur: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |_| {
                                    let val = (post_cmd)().trim().to_string();
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let update = GameConfigUpdate {
                                        post_exit_cmd: Some(if val.is_empty() { None } else { Some(val) }),
                                        ..Default::default()
                                    };
                                    let s = store.clone();
                                    s.call(
                                        async move { core.update_game_config(gid, update).await },
                                        |_, _| {},
                                    );
                                }
                            },
                        }
                    }
                }
            }
        }

        div { class: "settings-section",
            div { class: "settings-section-head",
                div {
                    h3 { class: "settings-section-title mono", "GRAPHICS & CONFIG AUDIT" }
                    div { class: "setting-desc", "Scan game installation and configuration for mod-breaking issues" }
                }
                button {
                    class: "btn btn-sm",
                    disabled: (auditing)(),
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let mut s = store.clone();
                            spawn(async move {
                                auditing.set(true);
                                match core.run_graphics_audit(gid).await {
                                    Ok(res) => {
                                        let count = res.len();
                                        audit_results.set(Some(res));
                                        if count == 0 {
                                            s.toast(ToastKind::Success, "Audit passed: No configuration issues detected");
                                        } else {
                                            s.toast(ToastKind::Info, format!("Audit found {count} warning(s)/advisory"));
                                        }
                                    }
                                    Err(e) => s.toast(ToastKind::Error, e),
                                }
                                auditing.set(false);
                            });
                        }
                    },
                    IconSvg { name: IconName::Search, size: 14 }
                    if (auditing)() { "AUDITING…" } else { "RUN AUDIT" }
                }
            }

            if let Some(res) = (audit_results)() {
                if res.is_empty() {
                    div { style: "padding: 10px; background: rgba(34, 197, 94, 0.1); border: 1px solid rgba(34, 197, 94, 0.4); border-radius: 4px; font-size: 11px; color: #4ade80;",
                        "✓ All graphics configuration checks passed. No issues detected."
                    }
                } else {
                    div { style: "display: flex; flex-direction: column; gap: 8px;",
                        for (idx, w) in res.iter().enumerate() {
                            {
                                let is_fixable = w.fixable;
                                let is_wuwa = game_id.to_lowercase().contains("wuwa") || game_id.to_lowercase().contains("wuthering");
                                let store = store.clone();
                                let gid = game_id.clone();
                                rsx! {
                                    div {
                                        key: "{idx}",
                                        style: "padding: 10px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px; display: flex; flex-direction: column; gap: 6px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div { style: "display: flex; align-items: center; gap: 8px;",
                                                span {
                                                    class: if w.level == "Error" { "tag tag-danger" } else if w.level == "Warning" { "tag tag-warn" } else { "tag" },
                                                    "{w.level}"
                                                }
                                                span { style: "font-size: 12px; font-weight: 600;", "{w.title}" }
                                            }
                                            if is_fixable && is_wuwa {
                                                button {
                                                    class: "btn btn-sm btn-accent",
                                                    disabled: (applying_tweaks)(),
                                                    onclick: move |_| {
                                                        let core = store.core.clone();
                                                        let gid = gid.clone();
                                                        let mut s = store.clone();
                                                        spawn(async move {
                                                            applying_tweaks.set(true);
                                                            match core.apply_wuwa_engine_tweaks(gid.clone()).await {
                                                                Ok(_) => {
                                                                    s.toast(ToastKind::Success, "Applied WuWa Engine.ini LOD & streaming tweaks");
                                                                    if let Ok(new_res) = core.run_graphics_audit(gid).await {
                                                                        audit_results.set(Some(new_res));
                                                                    }
                                                                }
                                                                Err(e) => s.toast(ToastKind::Error, e),
                                                            }
                                                            applying_tweaks.set(false);
                                                        });
                                                    },
                                                    if (applying_tweaks)() { "APPLYING…" } else { "APPLY WUWA ENGINE TWEAKS" }
                                                }
                                            }
                                        }
                                        div { style: "font-size: 11px; color: var(--text-sec); line-height: 1.4;",
                                            "{w.message}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        div { class: "settings-section",
            div { class: "settings-section-head",
                div {
                    h3 { class: "settings-section-title mono", "CREATOR DEV SUITE" }
                    div { class: "setting-desc", "In-game shader hunting mode, live d3d11_log.txt diagnostics, and gui_collect bridge" }
                }
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Search, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Hunting Mode (hunting = 2)" }
                        div { class: "setting-desc", "Enable in-game Numpad keys for hunting shaders and dumping IB/VB buffers" }
                    }
                }
                button {
                    class: if hunting_mode_on { "toggle-switch on" } else { "toggle-switch" },
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let update = GameConfigUpdate {
                                hunting_mode: Some(!hunting_mode_on),
                                ..Default::default()
                            };
                            let s = store.clone();
                            s.call(
                                async move { core.update_game_config(gid, update).await },
                                |_, _| {},
                            );
                        }
                    },
                    div { class: "toggle-knob" }
                }
            }

            div { class: "setting-row setting-row-col",
                div { class: "setting-label", style: "margin-bottom: 4px;", "gui_collect auto_generated.ini Path (Optional Bridge)" }
                div { style: "display: flex; align-items: center; gap: 8px;",
                    input {
                        class: "input mono",
                        style: "flex: 1; font-size: 11px;",
                        placeholder: "e.g. C:\\Tools\\gui_collect\\auto_generated.ini",
                        value: "{(gui_collect_path)()}",
                        oninput: move |e| gui_collect_path.set(e.value()),
                        onblur: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let val = (gui_collect_path)().trim().to_string();
                                let path_opt = if val.is_empty() { None } else { Some(std::path::PathBuf::from(val)) };
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let update = GameConfigUpdate {
                                    gui_collect_ini: Some(path_opt),
                                    ..Default::default()
                                };
                                let s = store.clone();
                                s.call(
                                    async move { core.update_game_config(gid, update).await },
                                    |_, _| {},
                                );
                            }
                        },
                    }
                }
            }

            div { style: "margin-top: 10px;",
                div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "font-size: 11px; font-weight: 600; color: var(--text);", "d3d11_log.txt Diagnostics" }
                    button {
                        class: "btn btn-sm",
                        disabled: (reading_log)(),
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    reading_log.set(true);
                                    match core.read_d3d11_diagnostics(gid).await {
                                        Ok(diags) => {
                                            let count = diags.len();
                                            log_diagnostics.set(Some(diags));
                                            if count == 0 {
                                                s.toast(ToastKind::Success, "No errors or warnings found in d3d11_log.txt");
                                            } else {
                                                s.toast(ToastKind::Info, format!("Parsed {count} diagnostic line(s)"));
                                            }
                                        }
                                        Err(e) => s.toast(ToastKind::Error, e),
                                    }
                                    reading_log.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Refresh, size: 13 }
                        if (reading_log)() { "READING LOG…" } else { "READ LOG DIAGNOSTICS" }
                    }
                }

                if let Some(diags) = (log_diagnostics)() {
                    if diags.is_empty() {
                        div { style: "padding: 8px 10px; background: rgba(34, 197, 94, 0.1); border: 1px solid rgba(34, 197, 94, 0.3); border-radius: 4px; font-size: 11px; color: #4ade80;",
                            "✓ Log is clean. No INI syntax errors, missing resources, or duplicate hashes found."
                        }
                    } else {
                        div { style: "max-height: 220px; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; padding: 6px; background: var(--bg); border: 1px solid var(--border); border-radius: 4px;",
                            for (idx, d) in diags.iter().enumerate() {
                                div {
                                    key: "{idx}",
                                    style: "padding: 6px 8px; background: var(--panel-2); border-radius: 3px; font-size: 11px; display: flex; flex-direction: column; gap: 3px;",
                                    div { style: "display: flex; align-items: center; gap: 6px;",
                                        span {
                                            class: if d.level == "Error" { "tag tag-danger" } else if d.level == "Warning" { "tag tag-warn" } else { "tag" },
                                            "{d.level}"
                                        }
                                        span { class: "mono", style: "font-size: 10px; color: var(--accent);", "{d.category}" }
                                        if let Some(f) = &d.file_hint {
                                            span { class: "mono", style: "font-size: 10px; color: var(--muted);", "[{f}]" }
                                        }
                                        if let Some(l) = d.line_number {
                                            span { class: "mono", style: "font-size: 10px; color: var(--muted);", ":{l}" }
                                        }
                                    }
                                    div { class: "mono", style: "font-size: 10px; color: var(--text-sec); word-break: break-all;",
                                        "{d.message}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        GameVoicePacksSection { game_id: game_id.clone() }

        GameEffectsSection { game_id: game_id.clone() }
    }
}

#[component]
fn GameVoicePacksSection(game_id: String) -> Element {
    let store = use_context::<Store>();
    let mut voice_packs = use_signal(Vec::<VoicePackStatus>::new);
    let mut checking = use_signal(|| false);
    let mut operating_pack = use_signal(|| None::<String>);
    let mut checked_once = use_signal(|| false);

    let packs = (voice_packs)();
    let is_checking = (checking)();
    let operating = (operating_pack)();
    let has_checked = (checked_once)();

    rsx! {
        div { class: "settings-section",
            div { class: "settings-section-head",
                div {
                    h3 { class: "settings-section-title mono", "VOICE-OVER PACKS & STORAGE" }
                    div { class: "setting-desc", "Manage downloaded language audio packages to optimize game install size" }
                }
                button {
                    class: "btn btn-sm",
                    disabled: is_checking || operating.is_some(),
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let mut s = store.clone();
                            spawn(async move {
                                checking.set(true);
                                match core.list_voice_packs(gid).await {
                                    Ok(res) => {
                                        let count = res.len();
                                        voice_packs.set(res);
                                        checked_once.set(true);
                                        s.toast(ToastKind::Info, format!("Found {count} voice-over pack option(s)"));
                                    }
                                    Err(e) => s.toast(ToastKind::Error, e),
                                }
                                checking.set(false);
                            });
                        }
                    },
                    IconSvg { name: IconName::Refresh, size: 14 }
                    if is_checking { "CHECKING…" } else { "CHECK VOICE PACKS" }
                }
            }

            if has_checked && packs.is_empty() {
                div { style: "padding: 10px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px; font-size: 11px; color: var(--muted);",
                    "No additional audio packages available for this title."
                }
            } else if !packs.is_empty() {
                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    for pack in packs.iter() {
                        {
                            let pid = pack.id.clone();
                            let pname = pack.name.clone();
                            let size_gb = format!("{:.2} GB", pack.size_bytes as f64 / 1024.0 / 1024.0 / 1024.0);
                            let is_installed = pack.installed;
                            let is_curr_operating = operating.as_deref() == Some(&pid);

                            let store = store.clone();
                            let gid = game_id.clone();
                            let pid_clone = pid.clone();

                            rsx! {
                                div {
                                    key: "{pid}",
                                    style: "padding: 10px 12px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px; display: flex; align-items: center; justify-content: space-between;",
                                    div { style: "display: flex; align-items: center; gap: 10px;",
                                        div {
                                            div { style: "font-size: 12px; font-weight: 600; color: var(--text);", "{pname}" }
                                            div { class: "mono", style: "font-size: 10px; color: var(--muted);", "Size: {size_gb}" }
                                        }
                                        if is_installed {
                                            span { class: "tag", style: "background: rgba(34, 197, 94, 0.15); color: #4ade80; border: 1px solid rgba(34, 197, 94, 0.4);", "INSTALLED" }
                                        } else {
                                            span { class: "tag tag-outline", "NOT INSTALLED" }
                                        }
                                    }

                                    if is_installed {
                                        button {
                                            class: "btn btn-sm btn-danger",
                                            disabled: operating.is_some(),
                                            onclick: {
                                                let pname = pname.clone();
                                                move |_| {
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let pid = pid_clone.clone();
                                                    let mut s = store.clone();
                                                    let pname = pname.clone();
                                                    spawn(async move {
                                                        operating_pack.set(Some(pid.clone()));
                                                        match core.remove_voice_pack(gid.clone(), pid.clone()).await {
                                                            Ok(freed) => {
                                                                let freed_mb = freed as f64 / 1024.0 / 1024.0;
                                                                s.toast(ToastKind::Success, format!("Removed {pname} (freed {freed_mb:.1} MB)"));
                                                                if let Ok(updated) = core.list_voice_packs(gid).await {
                                                                    voice_packs.set(updated);
                                                                }
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                        operating_pack.set(None);
                                                    });
                                                }
                                            },
                                            if is_curr_operating { "REMOVING…" } else { "REMOVE" }
                                        }
                                    } else {
                                        button {
                                            class: "btn btn-sm btn-accent",
                                            disabled: operating.is_some(),
                                            onclick: {
                                                let pname = pname.clone();
                                                move |_| {
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let pid = pid_clone.clone();
                                                    let mut s = store.clone();
                                                    let pname = pname.clone();
                                                    spawn(async move {
                                                        operating_pack.set(Some(pid.clone()));
                                                        s.toast(ToastKind::Info, format!("Downloading {pname}..."));
                                                        match core.install_voice_pack(gid.clone(), pid.clone()).await {
                                                            Ok(_) => {
                                                                s.toast(ToastKind::Success, format!("Voice pack {pname} download started"));
                                                                if let Ok(updated) = core.list_voice_packs(gid).await {
                                                                    voice_packs.set(updated);
                                                                }
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                        operating_pack.set(None);
                                                    });
                                                }
                                            },
                                            if is_curr_operating { "DOWNLOADING…" } else { "INSTALL" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn GameEffectsSection(game_id: String) -> Element {
    let store = use_context::<Store>();
    let mut syncing_hashes = use_signal(|| false);
    let mut installing_effect = use_signal(|| None::<String>);
    let mut refresh_trigger = use_signal(|| 0usize);

    let gid_res = game_id.clone();
    let core_res = store.core.clone();
    let trigger_val = (refresh_trigger)();
    let effects_resource = use_resource(move || {
        let core = core_res.clone();
        let gid = gid_res.clone();
        let _ = trigger_val;
        async move { core.list_effect_libraries(gid).await.unwrap_or_default() }
    });

    let effects = (effects_resource)().unwrap_or_default();
    let is_syncing = (syncing_hashes)();
    let installing = (installing_effect)();

    rsx! {
        div { class: "settings-section",
            div { class: "settings-section-head",
                div {
                    h3 { class: "settings-section-title mono", "SHARED EFFECTS & CHARACTER HASHES" }
                    div { class: "setting-desc", "Manage shared shader effect libraries and character identification hash database" }
                }
                button {
                    class: "btn btn-sm",
                    disabled: is_syncing,
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let mut s = store.clone();
                            spawn(async move {
                                syncing_hashes.set(true);
                                match core.sync_hash_db(gid).await {
                                    Ok(count) => s.toast(ToastKind::Success, format!("Synced {count} character hashes")),
                                    Err(e) => s.toast(ToastKind::Error, e),
                                }
                                syncing_hashes.set(false);
                            });
                        }
                    },
                    IconSvg { name: IconName::Refresh, size: 14 }
                    if is_syncing { "SYNCING…" } else { "SYNC HASHES" }
                }
            }

            if effects.is_empty() {
                div { class: "setting-desc", style: "padding: 8px 0; font-style: italic;",
                    "No shared effect libraries available for this game title."
                }
            }

            for eff in effects.iter() {
                {
                    let eff_id = eff.id.clone();
                    let eff_name = eff.name.clone();
                    let eff_desc = eff.description.clone();
                    let is_installed = eff.installed_version.is_some();
                    let version_str = eff.installed_version.clone().unwrap_or_else(|| "Not installed".to_string());
                    let is_enabled = eff.enabled;
                    let warnings = eff.duplicate_warnings.clone();
                    let texfx_cfg = eff.texfx_config;

                    rsx! {
                        div { key: "{eff_id}", class: "setting-row setting-row-col",
                            div { class: "setting-row-top",
                                div { class: "setting-row-left",
                                    div { class: "setting-icon",
                                        IconSvg { name: IconName::Folder, size: 18 }
                                    }
                                    div {
                                        div { style: "display: flex; align-items: center; gap: 8px;",
                                            span { class: "setting-label", "{eff_name}" }
                                            span {
                                                class: if is_installed { "tag" } else { "tag tag-outline" },
                                                style: "font-size: 10px; padding: 1px 6px;",
                                                "{version_str}"
                                            }
                                        }
                                        div { class: "setting-desc", "{eff_desc}" }
                                    }
                                }

                                div { style: "display: flex; align-items: center; gap: 10px;",
                                    button {
                                        class: "btn btn-sm",
                                        disabled: installing.as_deref() == Some(eff_id.as_str()),
                                        onclick: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let eid = eff_id.clone();
                                            let ename = eff_name.clone();
                                            move |_| {
                                                let core = store.core.clone();
                                                let gid = gid.clone();
                                                let eid = eid.clone();
                                                let ename = ename.clone();
                                                let mut s = store.clone();
                                                spawn(async move {
                                                    installing_effect.set(Some(eid.clone()));
                                                    match core.install_effect_library(gid, eid.clone()).await {
                                                        Ok(_) => {
                                                            s.toast(ToastKind::Success, format!("{ename} installed successfully"));
                                                            refresh_trigger.set(refresh_trigger() + 1);
                                                        }
                                                        Err(e) => s.toast(ToastKind::Error, e),
                                                    }
                                                    installing_effect.set(None);
                                                });
                                            }
                                        },
                                        if installing.as_deref() == Some(eff_id.as_str()) {
                                            "INSTALLING…"
                                        } else if is_installed {
                                            "UPDATE"
                                        } else {
                                            "INSTALL"
                                        }
                                    }

                                    if is_installed {
                                        button {
                                            class: if is_enabled { "toggle-switch on" } else { "toggle-switch" },
                                            title: if is_enabled { "Disable effect" } else { "Enable effect" },
                                            onclick: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                let eid = eff_id.clone();
                                                move |_| {
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let eid = eid.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        match core.toggle_effect_library(gid, eid, !is_enabled).await {
                                                            Ok(_) => refresh_trigger.set(refresh_trigger() + 1),
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                    });
                                                }
                                            },
                                            div { class: "toggle-knob" }
                                        }
                                    }
                                }
                            }

                            if !warnings.is_empty() {
                                div { class: "effect-warning-banner",
                                    div { style: "font-weight: 600; margin-bottom: 2px;",
                                        "⚠️ Duplicate effect INI detected in user mods (will conflict with managed library):"
                                    }
                                    ul { style: "margin: 0; padding-left: 16px; font-family: var(--mono); font-size: 10px;",
                                        for w in warnings.iter() {
                                            li { "{w}" }
                                        }
                                    }
                                }
                            }

                            if let Some(tcfg) = texfx_cfg {
                                div {
                                    style: "margin-top: 10px; padding: 10px; background: var(--panel-2); border: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px;",
                                    div { style: "display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-size: 11px; font-weight: 600;", "Uncensor Mode" }
                                            div { style: "font-size: 9px; color: var(--text-sec);", "Camera and render buffer uncensoring" }
                                        }
                                        select {
                                            class: "input select-sm",
                                            value: "{tcfg.uncensor}",
                                            onchange: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                let current_tcfg = tcfg;
                                                move |e: Event<FormData>| {
                                                    let val = e.value().parse::<u8>().unwrap_or(0);
                                                    let new_cfg = TexFxConfig {
                                                        uncensor: val,
                                                        seizure: current_tcfg.seizure,
                                                    };
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        match core.update_texfx_config(gid, new_cfg).await {
                                                            Ok(_) => {
                                                                s.toast(ToastKind::Success, "TexFx config updated");
                                                                refresh_trigger.set(refresh_trigger() + 1);
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                    });
                                                }
                                            },
                                            option { value: "0", "Off (0)" }
                                            option { value: "1", "Always On (1)" }
                                            option { value: "2", "Camera Proximity (2)" }
                                        }
                                    }

                                    div { style: "display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-size: 11px; font-weight: 600;", "Photosensitivity Protection" }
                                            div { style: "font-size: 9px; color: var(--text-sec);", "Seizure flash and burst suppression" }
                                        }
                                        button {
                                            class: if tcfg.seizure == 1 { "toggle-switch on" } else { "toggle-switch" },
                                            onclick: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                let current_tcfg = tcfg;
                                                move |_| {
                                                    let new_cfg = TexFxConfig {
                                                        uncensor: current_tcfg.uncensor,
                                                        seizure: if current_tcfg.seizure == 1 { 0 } else { 1 },
                                                    };
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        match core.update_texfx_config(gid, new_cfg).await {
                                                            Ok(_) => {
                                                                s.toast(ToastKind::Success, "TexFx config updated");
                                                                refresh_trigger.set(refresh_trigger() + 1);
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                    });
                                                }
                                            },
                                            div { class: "toggle-knob" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ProfileToggle(
    label: String,
    desc: String,
    checked: bool,
    field: String,
    game_id: String,
) -> Element {
    let store = use_context::<Store>();

    rsx! {
        div { class: "setting-row",
            div { class: "setting-row-left",
                div { class: "setting-icon",
                    IconSvg { name: IconName::Settings, size: 18 }
                }
                div {
                    div { class: "setting-label", "{label}" }
                    div { class: "setting-desc", "{desc}" }
                }
            }
            button {
                class: if checked { "toggle-switch on" } else { "toggle-switch" },
                onclick: {
                    let store = store.clone();
                    let field = field.clone();
                    let gid = game_id.clone();
                    move |_| {
                        let db = store.db(&gid);
                        let profile_id = db.as_ref()
                            .and_then(|d| d.games.get(&gid))
                            .map(|g| g.active_profile_id.clone())
                            .unwrap_or_default();
                        let core = store.core.clone();
                        let gid = gid.clone();
                        let field = field.clone();
                        let new_val = !checked;
                        let update = ProfileUpdate {
                            use_gamemode: if field == "use_gamemode" { Some(new_val) } else { None },
                            use_gamescope: if field == "use_gamescope" { Some(new_val) } else { None },
                            use_mangohud: if field == "use_mangohud" { Some(new_val) } else { None },
                            use_reshade: if field == "use_reshade" { Some(new_val) } else { None },
                            ..Default::default()
                        };
                        let s = store.clone();
                        s.call(
                            async move { core.update_profile(gid, profile_id, update).await },
                            |_, _| {},
                        );
                    }
                },
                div { class: "toggle-knob" }
            }
        }
    }
}

// ─── Management ──────────────────────────────────────────────────

#[component]
fn GameManagementTab(game_id: String) -> Element {
    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "DANGER ZONE" }
            p { class: "setting-desc", "These actions are destructive and cannot be undone." }

            DangerButton {
                label: "Wipe All Mods",
                desc: "Remove all installed mods for this game",
                game_id: game_id.clone(),
                action: DangerAction::WipeMods,
            }

            DangerButton {
                label: "Reset Profiles",
                desc: "Delete all profiles and create a fresh default",
                game_id: game_id.clone(),
                action: DangerAction::ResetProfiles,
            }

            DangerButton {
                label: "Remove Prefix",
                desc: "Delete the Wine/Proton prefix for this game",
                game_id: game_id.clone(),
                action: DangerAction::RemovePrefix,
            }

            DangerButton {
                label: "Full Uninstall",
                desc: "Remove game files, mods, prefix, and all data",
                game_id: game_id.clone(),
                action: DangerAction::Uninstall,
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DangerAction {
    WipeMods,
    ResetProfiles,
    RemovePrefix,
    Uninstall,
}

#[component]
fn DangerButton(label: String, desc: String, game_id: String, action: DangerAction) -> Element {
    let store = use_context::<Store>();
    let mut confirmed = use_signal(|| false);
    let is_confirmed = (confirmed)();

    rsx! {
        div { class: "danger-row",
            div {
                div { class: "setting-label", "{label}" }
                div { class: "setting-desc", "{desc}" }
            }
            if !is_confirmed {
                button {
                    class: "btn btn-danger",
                    onclick: move |_| confirmed.set(true),
                    IconSvg { name: IconName::Trash, size: 14 }
                    "{label}"
                }
            } else {
                div { class: "danger-confirm",
                    span { class: "setting-desc", "Are you sure?" }
                    button {
                        class: "btn-link",
                        onclick: move |_| confirmed.set(false),
                        "CANCEL"
                    }
                    button {
                        class: "btn btn-danger",
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let s = store.clone();
                                match action {
                                    DangerAction::WipeMods => s.call(
                                        async move { core.wipe_game_mods(gid).await },
                                        |s, _| s.toast(ToastKind::Success, "Mods wiped"),
                                    ),
                                    DangerAction::ResetProfiles => s.call(
                                        async move { core.reset_game_profiles(gid).await },
                                        |s, _| s.toast(ToastKind::Success, "Profiles reset"),
                                    ),
                                    DangerAction::RemovePrefix => s.call(
                                        async move { core.remove_game_prefix(gid).await },
                                        |s, _| s.toast(ToastKind::Success, "Prefix removed"),
                                    ),
                                    DangerAction::Uninstall => s.call(
                                        async move { core.uninstall_game_files(gid).await },
                                        |s, _| s.toast(ToastKind::Success, "Game uninstalled"),
                                    ),
                                }
                                confirmed.set(false);
                            }
                        },
                        "CONFIRM"
                    }
                }
            }
        }
    }
}
