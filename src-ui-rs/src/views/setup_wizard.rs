use dioxus::prelude::*;
use yago_app::{IdentifiedGame, SetupStatus};

use crate::icons::{IconName, IconSvg};
use crate::picker::pick_folder;
use crate::state::{Store, ToastKind};

const TITLES: [&str; 7] = [
    "STORAGE HOME",
    "GAME LIBRARY",
    "IDENTIFY TITLES",
    "ADVANCED PATHS",
    "COMPAT TOOLS",
    "MOD ASSETS",
    "EVERYTHING READY",
];

#[component]
pub fn SetupWizard() -> Element {
    let store = use_context::<Store>();
    let mut step = use_signal(|| 0u8);
    let settings = (store.settings)();

    let storage_path = use_signal(|| settings.yago_storage_path.to_string_lossy().to_string());
    let games_path = use_signal(|| settings.default_games_path.to_string_lossy().to_string());
    let mods_path = use_signal(|| settings.mods_path.to_string_lossy().to_string());
    let runners_path = use_signal(|| settings.runners_path.to_string_lossy().to_string());
    let prefixes_path = use_signal(|| settings.prefixes_path.to_string_lossy().to_string());
    let cache_path = use_signal(|| settings.cache_path.to_string_lossy().to_string());
    let mut proton_path = use_signal(|| {
        settings
            .steam_compat_tools_path
            .to_string_lossy()
            .to_string()
    });

    let mut status = use_signal(|| Option::<SetupStatus>::None);
    let mut discovered = use_signal(Vec::<IdentifiedGame>::new);
    let scanning = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut download_error = use_signal(|| Option::<String>::None);

    {
        let core = store.core.clone();
        use_future(move || {
            let core = core.clone();
            async move {
                if let Ok(s) = core.get_setup_status().await {
                    if let Some(p) = &s.detected_steam_path {
                        if (proton_path)().is_empty() {
                            proton_path.set(p.clone());
                        }
                    }
                    status.set(Some(s));
                }
            }
        });
    }

    let apply_paths = move |s: &mut Store| {
        let mut settings = (s.settings)();
        settings.yago_storage_path = (storage_path)().into();
        settings.default_games_path = (games_path)().into();
        settings.mods_path = (mods_path)().into();
        settings.runners_path = (runners_path)().into();
        settings.prefixes_path = (prefixes_path)().into();
        settings.cache_path = (cache_path)().into();
        s.save_settings(settings);
    };
    let mut save_store0 = store.clone();
    let mut save_store1 = store.clone();
    let mut save_store3 = store.clone();
    let mut confirm_store = store.clone();

    let scan_store1 = store.clone();
    let scan_store2 = store.clone();

    let detect_store = store.clone();
    let auto_detect = move |_| {
        let core = detect_store.core.clone();
        let mut s = detect_store.clone();
        spawn(async move {
            match core.detect_steam_proton_path().await {
                Ok(Some(p)) => proton_path.set(p),
                Ok(None) => s.toast(
                    ToastKind::Info,
                    "No Steam Proton directories found. Select manually.",
                ),
                Err(e) => s.toast(ToastKind::Error, e),
            }
        });
    };

    let mut proton_store = store.clone();
    let download_proton = move |_| {
        download_error.set(None);
        proton_store.proton_progress.set(None);
        let core = proton_store.core.clone();
        let mut s = proton_store.clone();
        let task = s.task_start("Proton GE", "Downloading runner");
        spawn(async move {
            match core.download_proton().await {
                Ok(()) => s.task_finish(task, true, "Runner ready"),
                Err(e) => {
                    s.task_finish(task, false, e.clone());
                    download_error.set(Some(e));
                }
            }
        });
    };

    let mut libs_store = store.clone();
    let install_libs = move |_| {
        download_error.set(None);
        libs_store.loader_progress.write().remove("common");
        let core = libs_store.core.clone();
        let mut s = libs_store.clone();
        let task = s.task_start("Mod assets", "Installing common loaders");
        spawn(async move {
            match core.install_common_libs().await {
                Ok(()) => s.task_finish(task, true, "Loaders ready"),
                Err(e) => {
                    s.task_finish(task, false, e.clone());
                    download_error.set(Some(e));
                }
            }
        });
    };

    let finish_store = store.clone();
    let finish = move |_| {
        busy.set(true);
        let mut s = finish_store.clone();
        let core = s.core.clone();
        spawn(async move {
            let mut settings = (s.settings)();
            settings.yago_storage_path = (storage_path)().into();
            settings.default_games_path = (games_path)().into();
            settings.mods_path = (mods_path)().into();
            settings.runners_path = (runners_path)().into();
            settings.prefixes_path = (prefixes_path)().into();
            settings.cache_path = (cache_path)().into();
            let saved = core.update_settings(settings).await;
            busy.set(false);
            if let Err(e) = saved {
                s.toast(ToastKind::Error, format!("Failed to save settings: {e}"));
            } else {
                s.setup_done.set(Some(true));
                s.refresh().await;
                s.toast(
                    ToastKind::Success,
                    "YAGO initialized. Welcome to the library.",
                );
            }
        });
    };

    let skip_store = store.clone();
    let skip_wizard = move |_| {
        let mut s = skip_store.clone();
        let core = s.core.clone();
        spawn(async move {
            let settings = (s.settings)();
            let _ = core.update_settings(settings).await;
            s.setup_done.set(Some(true));
            s.refresh().await;
            s.toast(
                ToastKind::Success,
                "Default settings applied. Welcome to YAGO.",
            );
        });
    };

    let current = (step)();
    let st = (status)();
    let has_runners = st.as_ref().map(|s| s.has_runners).unwrap_or(false);
    let is_scanning = (scanning)();
    let found = (discovered)();
    let is_busy = (busy)();
    let proton_val = (proton_path)();
    let proton_prog = (store.proton_progress)();
    let loader_prog = (store.loader_progress)().get("common").cloned();
    let dl_err = (download_error)();

    let add_store = store.clone();

    rsx! {
        div { class: "setup-wrap",
            div { class: "setup-card",
                div { class: "setup-head",
                    IconSvg { name: IconName::Shield, size: 24 }
                    h1 { class: "setup-title", "{TITLES[current as usize]}" }
                    div { class: "setup-dots",
                        for i in 0..7u8 {
                            span {
                                key: "{i}",
                                class: if i == current { "setup-dot active" } else if i < current { "setup-dot done" } else { "setup-dot" },
                            }
                        }
                    }
                }

                div { class: "setup-body",
                    if current == 0 {
                        p { class: "wizard-note",
                            "Where YAGO keeps its library, loaders, runners and prefixes. Leave empty to use the default app-data location."
                        }
                        PathField { label: "STORAGE HOME", value: storage_path, placeholder: "Default (App Data)" }
                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: skip_wizard,
                                "SKIP TO LIBRARY"
                            }
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| { apply_paths(&mut save_store0); step.set(1); },
                                "NEXT"
                                IconSvg { name: IconName::ChevronRight, size: 14 }
                            }
                        }
                        button {
                            class: "link-btn mono",
                            onclick: move |_| step.set(3),
                            "ADVANCED PATH OVERRIDES"
                        }
                    }

                    if current == 1 {
                        p { class: "wizard-note",
                            "Point to your primary games directory for automatic discovery."
                        }
                        PathField { label: "GAMES ROOT", value: games_path, placeholder: "/home/user/Games" }
                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: move |_| step.set(0),
                                "BACK"
                            }
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| {
                                    apply_paths(&mut save_store1);
                                    if (games_path)().is_empty() {
                                        step.set(if has_runners { 5 } else { 4 });
                                    } else {
                                        step.set(2);
                                        run_scan(scan_store1.clone(), games_path, scanning, discovered);
                                    }
                                },
                                "START DISCOVERY"
                                IconSvg { name: IconName::ChevronRight, size: 14 }
                            }
                        }
                    }

                    if current == 2 {
                        p { class: "wizard-note mono",
                            if is_scanning { "SCANNING LIBRARY…" } else { "{found.len()} TITLES FOUND" }
                        }
                        div { class: "found-list",
                            if is_scanning {
                                p { class: "wizard-note", "Looking for supported titles…" }
                            } else if found.is_empty() {
                                p { class: "wizard-note", "No supported games found in this root." }
                            } else {
                                for g in found.iter() {
                                    div { class: "found-row", key: "{g.id}",
                                        div { class: "found-initial mono", "{g.logo_initial}" }
                                        div { class: "found-text",
                                            span { class: "found-name", "{g.name}" }
                                            span { class: "found-path mono", "{g.install_path}" }
                                        }
                                        button {
                                            class: "btn btn-accent",
                                            onclick: {
                                                let path = g.install_path.clone();
                                                let gid = g.id.clone();
                                                let name = g.name.clone();
                                                let add_store = add_store.clone();
                                                move |_| {
                                                    let core = add_store.core.clone();
                                                    let mut s = add_store.clone();
                                                    let path = path.clone();
                                                    let gid = gid.clone();
                                                    let name = name.clone();
                                                    spawn(async move {
                                                        match core.add_game(path).await {
                                                            Ok(_) => {
                                                                s.toast(ToastKind::Success, format!("{name} added"));
                                                                discovered.write().retain(|e| e.id != gid);
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                    });
                                                }
                                            },
                                            "ADD"
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: move |_| step.set(1),
                                "BACK"
                            }
                            button {
                                class: "btn",
                                disabled: is_scanning,
                                onclick: move |_| run_scan(scan_store2.clone(), games_path, scanning, discovered),
                                IconSvg { name: IconName::Refresh, size: 14 }
                                "RESCAN"
                            }
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| step.set(if has_runners { 5 } else { 4 }),
                                "CONTINUE"
                                IconSvg { name: IconName::ChevronRight, size: 14 }
                            }
                        }
                    }

                    if current == 3 {
                        PathField { label: "MODS PATH", value: mods_path, placeholder: "Use storage default" }
                        PathField { label: "RUNNERS PATH", value: runners_path, placeholder: "Use storage default" }
                        PathField { label: "PREFIXES PATH", value: prefixes_path, placeholder: "Use storage default" }
                        PathField { label: "CACHE PATH", value: cache_path, placeholder: "Use storage default" }
                        div { class: "modal-actions",
                            button {
                                class: "btn btn-accent",
                                onclick: move |_| { apply_paths(&mut save_store3); step.set(0); },
                                "SAVE OVERRIDES"
                            }
                        }
                    }

                    if current == 4 {
                        p { class: "wizard-note",
                            "Proton GE is required to run Windows titles on Linux. Download it, or link your existing Steam compatibility tools."
                        }
                        if !proton_val.is_empty() {
                            div { class: "linked-path",
                                span { class: "field-label mono", "LINKED PATH" }
                                span { class: "mono linked-path-val", "{proton_val}" }
                            }
                        }
                        if let Some(p) = proton_prog.as_ref().filter(|p| p.status != "Done") {
                            div { class: "setup-progress",
                                div { class: "download-head",
                                    span { class: "mono", "{p.status}" }
                                    span { class: "mono", "{(p.progress * 100.0) as u32}%" }
                                }
                                div { class: "progress-track",
                                    div { class: "progress-fill", style: "width: {(p.progress * 100.0).clamp(0.0, 100.0)}%" }
                                }
                            }
                        }
                        if let Some(err) = dl_err.as_ref() {
                            p { class: "error-note mono", "{err}" }
                        }
                        div { class: "add-actions",
                            button {
                                class: "btn btn-accent",
                                onclick: download_proton,
                                IconSvg { name: IconName::Download, size: 15 }
                                "DOWNLOAD PROTON GE"
                            }
                            button {
                                class: "btn",
                                onclick: auto_detect,
                                IconSvg { name: IconName::Search, size: 15 }
                                "AUTO DETECT"
                            }
                            button {
                                class: "btn",
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Some(p) = pick_folder("Select compatibilitytools.d or Proton folder").await {
                                            proton_path.set(p.to_string_lossy().to_string());
                                        }
                                    });
                                },
                                IconSvg { name: IconName::Folder, size: 15 }
                                "MANUAL"
                            }
                        }
                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: move |_| step.set(2),
                                "BACK"
                            }
                            if !proton_val.is_empty() {
                                button {
                                    class: "btn",
                                    onclick: move |_| proton_path.set(String::new()),
                                    "RESET"
                                }
                                button {
                                    class: "btn btn-accent",
                                    onclick: move |_| {
                                        let mut s = (confirm_store.settings)();
                                        s.steam_compat_tools_path = (proton_path)().into();
                                        confirm_store.save_settings(s);
                                        step.set(5);
                                    },
                                    "CONFIRM PATH"
                                }
                            }
                            if proton_prog.as_ref().is_some_and(|p| p.status == "Done") || dl_err.is_some() {
                                button {
                                    class: "btn btn-accent",
                                    onclick: move |_| step.set(5),
                                    if dl_err.is_some() { "SKIP" } else { "CONTINUE" }
                                    IconSvg { name: IconName::ChevronRight, size: 14 }
                                }
                            }
                        }
                    }

                    if current == 5 {
                        p { class: "wizard-note",
                            "Install common modding libraries (3DMigoto loaders and ReShade) to enable full mod support."
                        }
                        if let Some(p) = loader_prog.as_ref().filter(|p| p.status != "Done") {
                            div { class: "setup-progress",
                                div { class: "download-head",
                                    span { class: "mono", "{p.status}" }
                                    span { class: "mono", "{(p.progress * 100.0) as u32}%" }
                                }
                                div { class: "progress-track",
                                    div { class: "progress-fill", style: "width: {(p.progress * 100.0).clamp(0.0, 100.0)}%" }
                                }
                            }
                        }
                        if loader_prog.as_ref().is_some_and(|p| p.status == "Done") {
                            div { class: "ready-badge mono",
                                IconSvg { name: IconName::Check, size: 16 }
                                "LOADERS READY"
                            }
                        }
                        if let Some(err) = dl_err.as_ref() {
                            p { class: "error-note mono", "{err}" }
                        }
                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: move |_| step.set(4),
                                "BACK"
                            }
                            button {
                                class: "btn btn-accent",
                                onclick: install_libs,
                                IconSvg { name: IconName::Download, size: 15 }
                                if loader_prog.is_some() { "RETRY INSTALL" } else { "SETUP COMMON ASSETS" }
                            }
                            if loader_prog.as_ref().is_some_and(|p| p.status == "Done") {
                                button {
                                    class: "btn btn-accent",
                                    onclick: move |_| step.set(6),
                                    "FINALIZE"
                                    IconSvg { name: IconName::ChevronRight, size: 14 }
                                }
                            } else {
                                button {
                                    class: "btn",
                                    onclick: move |_| step.set(6),
                                    "SKIP FOR NOW"
                                    IconSvg { name: IconName::ChevronRight, size: 14 }
                                }
                            }
                        }
                    }

                    if current == 6 {
                        div { class: "wizard-confirm",
                            IconSvg { name: IconName::Check, size: 44 }
                            h3 { "INITIALIZATION COMPLETE" }
                            p { class: "wizard-note", "YAGO is ready. Your library and settings are saved." }
                        }
                        div { class: "modal-actions",
                            button {
                                class: "btn btn-accent",
                                disabled: is_busy,
                                onclick: finish,
                                if is_busy { "SAVING SETTINGS…" } else { "FINISH SETUP & ENTER LIBRARY" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PathField(label: String, value: Signal<String>, placeholder: String) -> Element {
    let title = format!("Select {label}");
    rsx! {
        div { class: "path-field",
            label { class: "field-label mono", "{label}" }
            div { class: "path-row",
                input {
                    class: "input",
                    r#type: "text",
                    value: "{value}",
                    placeholder: "{placeholder}",
                    oninput: move |evt| value.set(evt.value()),
                }
                button {
                    class: "btn",
                    title: "Browse",
                    onclick: move |_| {
                        let title = title.clone();
                        spawn(async move {
                            if let Some(p) = pick_folder(&title).await {
                                value.set(p.to_string_lossy().to_string());
                            }
                        });
                    },
                    IconSvg { name: IconName::Folder, size: 15 }
                }
            }
        }
    }
}

fn run_scan(
    store: Store,
    games_path: Signal<String>,
    mut scanning: Signal<bool>,
    mut discovered: Signal<Vec<IdentifiedGame>>,
) {
    let root = (games_path)();
    if root.is_empty() {
        return;
    }
    scanning.set(true);
    let core = store.core.clone();
    let mut s = store.clone();
    spawn(async move {
        let result = async {
            let found = core.recursive_scan_path(root).await?;
            let mut out = Vec::new();
            for d in found {
                if let Ok(g) = core
                    .identify_game(d.path.to_string_lossy().to_string())
                    .await
                {
                    if !out.iter().any(|e: &IdentifiedGame| e.id == g.id) {
                        out.push(g);
                    }
                }
            }
            Ok::<_, String>(out)
        }
        .await;
        scanning.set(false);
        match result {
            Ok(list) => discovered.set(list),
            Err(e) => s.toast(ToastKind::Error, format!("Discovery failed: {e}")),
        }
    });
}
