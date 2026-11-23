use dioxus::prelude::*;

use crate::icons::{IconName, IconSvg};
use crate::picker::pick_folder;
use crate::state::{Store, ToastKind};
use crate::themes::THEMES;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    Paths,
    Runners,
    Repos,
}

#[component]
pub fn SettingsView() -> Element {
    let mut active_tab = use_signal(|| Tab::General);

    let tab = (active_tab)();

    rsx! {
        div { class: "settings-view",
            div { class: "settings-header",
                div { class: "settings-title",
                    IconSvg { name: IconName::Settings, size: 24 }
                    h1 { "APP SETTINGS" }
                }
            }

            div { class: "settings-tabs",
                for &(t, label) in &[(Tab::General, "GENERAL"), (Tab::Paths, "PATHS"), (Tab::Runners, "RUNNERS"), (Tab::Repos, "REPOS")] {
                    button {
                        class: if tab == t { "seg-btn active" } else { "seg-btn" },
                        onclick: move |_| active_tab.set(t),
                        "{label}"
                    }
                }
            }

            div { class: "settings-body",
                match tab {
                    Tab::General => rsx! { GeneralTab {} },
                    Tab::Paths => rsx! { PathsTab {} },
                    Tab::Runners => rsx! { RunnersTab {} },
                    Tab::Repos => rsx! { ReposTab {} },
                }
            }
        }
    }
}

// ─── General ─────────────────────────────────────────────────────

#[component]
fn GeneralTab() -> Element {
    let store = use_context::<Store>();
    let settings = (store.settings)();
    let app_config = (store.app_config)();
    let current_theme = app_config.theme.clone();
    let mut update_status = use_signal(|| None::<yago_app::AppUpdateInfo>);
    let mut checking_update = use_signal(|| false);

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "APPLICATION BEHAVIOR" }

            ToggleRow {
                icon: IconName::Shield,
                label: "Stream Safe Mode",
                description: "Automatically hide or blur NSFW content",
                checked: settings.stream_safe,
                ontoggle: {
                    let store = store.clone();
                    move |_| {
                        let mut s = store.clone();
                        let mut new_settings = (s.settings)();
                        new_settings.stream_safe = !new_settings.stream_safe;
                        s.save_settings(new_settings);
                    }
                },
            }

            if settings.stream_safe {
                div { class: "setting-row",
                    div { class: "setting-row-left",
                        div { class: "setting-icon",
                            IconSvg { name: IconName::Shield, size: 18 }
                        }
                        div {
                            div { class: "setting-label", "NSFW Filter Style" }
                            div { class: "setting-desc", "Choose whether to blur or completely hide flagged content" }
                        }
                    }
                    select {
                        class: "input select-sm",
                        value: "{settings.nsfw_behavior}",
                        onchange: {
                            let store = store.clone();
                            move |evt: Event<FormData>| {
                                let mut s = store.clone();
                                let mut new_settings = (s.settings)();
                                new_settings.nsfw_behavior = evt.value();
                                s.save_settings(new_settings);
                            }
                        },
                        option { value: "blur", "Blur NSFW Previews" }
                        option { value: "hide", "Hide NSFW Content Completely" }
                    }
                }
            }

            ToggleRow {
                icon: IconName::Settings,
                label: "Suppress $creditinfo Banners Globally",
                description: "Silently suppress on-screen credit banners across all games and mods",
                checked: settings.suppress_creditinfo_global,
                ontoggle: {
                    let store = store.clone();
                    move |_| {
                        let mut s = store.clone();
                        let mut new_settings = (s.settings)();
                        new_settings.suppress_creditinfo_global =
                            !new_settings.suppress_creditinfo_global;
                        s.save_settings(new_settings);
                    }
                },
            }

            ToggleRow {
                icon: IconName::Deploy,
                label: "Close on Launch",
                description: "Exit YAGO to tray when a game starts",
                checked: settings.close_on_launch,
                ontoggle: {
                    let store = store.clone();
                    move |_| {
                        let mut s = store.clone();
                        let mut new_settings = (s.settings)();
                        new_settings.close_on_launch = !new_settings.close_on_launch;
                        s.save_settings(new_settings);
                    }
                },
            }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Grid, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "Language" }
                        div { class: "setting-desc", "Select your preferred display language" }
                    }
                }
                select {
                    class: "input select-sm",
                    value: "{settings.language}",
                    onchange: {
                        let store = store.clone();
                        move |evt: Event<FormData>| {
                            let mut s = store.clone();
                            let mut new_settings = (s.settings)();
                            new_settings.language = evt.value();
                            s.save_settings(new_settings);
                        }
                    },
                    option { value: "en-US", "English (US)" }
                    option { value: "ja-JP", "日本語 (Japanese)" }
                    option { value: "zh-CN", "简体中文 (Chinese)" }
                }
            }
        }

        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "THEME & APPEARANCE" }

            div { class: "theme-grid",
                for theme in THEMES.iter() {
                    {
                        let tid = theme.id.to_string();
                        let selected = current_theme == tid;
                        rsx! {
                            button {
                                key: "{tid}",
                                class: if selected { "theme-card selected" } else { "theme-card" },
                                onclick: {
                                    let store = store.clone();
                                    let tid = tid.clone();
                                    move |_| {
                                        let mut config = (store.app_config)();
                                        config.theme = tid.clone();
                                        let core = store.core.clone();
                                        let config2 = config.clone();
                                        store.clone().call(
                                            async move { core.update_app_config(config2).await },
                                            |_, _| {},
                                        );
                                    }
                                },
                                div { class: "theme-swatch theme-{theme.id}" }
                                div { class: "theme-meta",
                                    span { class: "theme-name", "{theme.name}" }
                                    span { class: "theme-engine mono", "{theme.engine}" }
                                }
                                if selected {
                                    div { class: "theme-check",
                                        IconSvg { name: IconName::Check, size: 14 }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "APPLICATION & UPDATES" }

            div { class: "setting-row",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Info, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "YAGO Version" }
                        div { class: "setting-desc mono", "v{env!(\"CARGO_PKG_VERSION\")}" }
                    }
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    button {
                        class: "btn btn-sm",
                        disabled: (checking_update)(),
                        onclick: {
                            let store = store.clone();
                            move |_| {
                                let core = store.core.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    checking_update.set(true);
                                    match core.check_app_update().await {
                                        Ok(info) => {
                                            if info.has_update {
                                                s.toast(ToastKind::Success, format!("Update available: v{}", info.latest_version));
                                            } else {
                                                s.toast(ToastKind::Info, "YAGO is up-to-date");
                                            }
                                            update_status.set(Some(info));
                                        }
                                        Err(e) => s.toast(ToastKind::Error, format!("Update check failed: {e}")),
                                    }
                                    checking_update.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Download, size: 14 }
                        if (checking_update)() { "CHECKING…" } else { "CHECK FOR UPDATES" }
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: {
                            let store = store.clone();
                            move |_| {
                                let core = store.core.clone();
                                let logs_dir = core.app_data_dir.join("logs").display().to_string();
                                let s = store.clone();
                                s.call(
                                    async move { core.open_path(logs_dir).await },
                                    |_, _| {},
                                );
                            }
                        },
                        IconSvg { name: IconName::Folder, size: 14 }
                        "OPEN LOGS FOLDER"
                    }
                    button {
                        class: "btn btn-sm",
                        onclick: {
                            let store = store.clone();
                            move |_| {
                                let core = store.core.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    match core.export_bug_report(None).await {
                                        Ok(zip_path) => {
                                            s.toast(ToastKind::Success, format!("Created bug report: {zip_path}"));
                                            let _ = core.open_path(zip_path).await;
                                        }
                                        Err(e) => s.toast(ToastKind::Error, format!("Bug report failed: {e}")),
                                    }
                                });
                            }
                        },
                        IconSvg { name: IconName::Download, size: 14 }
                        "CREATE BUG REPORT (.ZIP)"
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
                        IconSvg { name: IconName::Launch, size: 14 }
                        "REPORT ISSUE ON GITHUB"
                    }
                }
            }

            if let Some(info) = (update_status)() {
                div {
                    style: "margin-top: 8px; padding: 12px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px; display: flex; flex-direction: column; gap: 6px;",
                    div { style: "display: flex; align-items: center; justify-content: space-between;",
                        span { class: "mono", style: "font-weight: 600; font-size: 12px; color: var(--accent);",
                            if info.has_update {
                                "{info.latest_version} (Update Available)"
                            } else {
                                "YAGO is up-to-date"
                            }
                        }
                        if let Some(url) = &info.release_url {
                            if info.has_update {
                                button {
                                    class: "btn btn-sm btn-accent",
                                    onclick: {
                                        let u = url.clone();
                                        let store = store.clone();
                                        move |_| {
                                            let core = store.core.clone();
                                            let url_str = u.clone();
                                            spawn(async move {
                                                let _ = core.open_path(url_str).await;
                                            });
                                        }
                                    },
                                    "VIEW ON GITHUB"
                                }
                            }
                        }
                    }
                    if let Some(notes) = &info.notes {
                        if !notes.trim().is_empty() {
                            div {
                                class: "mono",
                                style: "font-size: 11px; color: var(--text-sec); max-height: 120px; overflow-y: auto; white-space: pre-wrap;",
                                "{notes}"
                            }
                        }
                    }
                }
            }
        }

        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "MAINTENANCE" }

            button {
                class: "btn btn-danger btn-block",
                onclick: {
                    let store = store.clone();
                    move |_| {
                        let core = store.core.clone();
                        let s = store.clone();
                        s.call(
                            async move { core.force_reset_state().await },
                            |s, _| s.toast(ToastKind::Success, "App state has been reset"),
                        );
                    }
                },
                IconSvg { name: IconName::Refresh, size: 15 }
                "FORCE RESET APP STATE"
            }
        }
    }
}

// ─── Paths ───────────────────────────────────────────────────────

#[component]
fn PathsTab() -> Element {
    let store = use_context::<Store>();
    let settings = (store.settings)();

    let paths: Vec<(&str, &str, String)> = vec![
        (
            "Steam Compatibility Tools",
            "Path to Steam 'compatibilitytools.d' directory",
            settings.steam_compat_tools_path.display().to_string(),
        ),
        (
            "Global Wine Prefix",
            "Default prefix for games without a specific one",
            settings.wine_prefix_path.display().to_string(),
        ),
        (
            "YAGO Storage Path",
            "Where YAGO stores databases, metadata, and logs",
            settings.yago_storage_path.display().to_string(),
        ),
        (
            "Default Games Folder",
            "Default location for new game installs",
            settings.default_games_path.display().to_string(),
        ),
        (
            "Mods Storage",
            "Where YAGO stores and manages your mods",
            settings.mods_path.display().to_string(),
        ),
        (
            "Runners (Proton/Wine)",
            "Folder for compatibility layers",
            settings.runners_path.display().to_string(),
        ),
        (
            "Game Prefixes",
            "Base directory for game-specific prefixes",
            settings.prefixes_path.display().to_string(),
        ),
        (
            "Cache Directory",
            "Temporary downloads and extracted files",
            settings.cache_path.display().to_string(),
        ),
    ];

    let field_keys = [
        "steam_compat_tools_path",
        "wine_prefix_path",
        "yago_storage_path",
        "default_games_path",
        "mods_path",
        "runners_path",
        "prefixes_path",
        "cache_path",
    ];

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "SYSTEM DIRECTORIES" }

            for (i, (label, desc, value)) in paths.iter().enumerate() {
                PathRow {
                    key: "{field_keys[i]}",
                    label: label.to_string(),
                    description: desc.to_string(),
                    value: value.clone(),
                    field_key: field_keys[i].to_string(),
                }
            }
        }
    }
}

#[component]
fn PathRow(label: String, description: String, value: String, field_key: String) -> Element {
    let store = use_context::<Store>();
    let mut editing = use_signal(|| false);
    let mut local_val = use_signal(|| value.clone());
    let is_editing = (editing)();

    rsx! {
        div { class: "setting-row setting-row-col",
            div { class: "setting-row-top",
                div { class: "setting-row-left",
                    div { class: "setting-icon",
                        IconSvg { name: IconName::Folder, size: 18 }
                    }
                    div {
                        div { class: "setting-label", "{label}" }
                        div { class: "setting-desc", "{description}" }
                    }
                }
                if !is_editing {
                    div { class: "setting-row-actions",
                        if !value.is_empty() {
                            button {
                                class: "icon-btn",
                                title: "Open in File Manager",
                                onclick: {
                                    let open_store = store.clone();
                                    let p = value.clone();
                                    move |_| {
                                        let core = open_store.core.clone();
                                        let p = p.clone();
                                        open_store.call(async move { core.open_path(p).await }, |_, _| {});
                                    }
                                },
                                IconSvg { name: IconName::Folder, size: 15 }
                            }
                        }
                        button {
                            class: "btn-link",
                            onclick: {
                                let val = value.clone();
                                move |_| {
                                    local_val.set(val.clone());
                                    editing.set(true);
                                }
                            },
                            "EDIT"
                        }
                    }
                }
            }
            if is_editing {
                div { class: "path-editor",
                    div { class: "path-editor-row",
                        input {
                            class: "input mono",
                            r#type: "text",
                            value: "{(local_val)()}",
                            oninput: move |evt| local_val.set(evt.value()),
                        }
                        button {
                            class: "btn",
                            onclick: move |_| {
                                spawn(async move {
                                    if let Some(path) = pick_folder("Select Folder").await {
                                        local_val.set(path.display().to_string());
                                    }
                                });
                            },
                            IconSvg { name: IconName::Folder, size: 16 }
                        }
                    }
                    div { class: "path-editor-actions",
                        button {
                            class: "btn-link",
                            onclick: move |_| editing.set(false),
                            "CANCEL"
                        }
                        button {
                            class: "btn btn-accent",
                            onclick: {
                                let store = store.clone();
                                let field_key = field_key.clone();
                                move |_| {
                                    let val = (local_val)();
                                    let field_key = field_key.clone();
                                    let mut s = store.clone();
                                    let mut new_settings = (s.settings)();
                                    let path = std::path::PathBuf::from(&val);
                                    match field_key.as_str() {
                                        "steam_compat_tools_path" => new_settings.steam_compat_tools_path = path,
                                        "wine_prefix_path" => new_settings.wine_prefix_path = path,
                                        "yago_storage_path" => new_settings.yago_storage_path = path,
                                        "default_games_path" => new_settings.default_games_path = path,
                                        "mods_path" => new_settings.mods_path = path,
                                        "runners_path" => new_settings.runners_path = path,
                                        "prefixes_path" => new_settings.prefixes_path = path,
                                        "cache_path" => new_settings.cache_path = path,
                                        _ => {}
                                    }
                                    s.save_settings(new_settings);
                                    editing.set(false);
                                }
                            },
                            "SAVE"
                        }
                    }
                }
            } else {
                div { class: "setting-value mono", "{value}" }
            }
        }
    }
}

// ─── Runners ─────────────────────────────────────────────────────

#[component]
fn RunnersTab() -> Element {
    let store = use_context::<Store>();
    let settings = (store.settings)();
    let proton_progress = (store.proton_progress)();

    let core = store.core.clone();
    let mut runners = use_resource(move || {
        let core = core.clone();
        async move { core.list_runners().await.unwrap_or_default() }
    });

    let runner_list: Vec<String> = (runners)().unwrap_or_default();

    rsx! {
        div { class: "settings-section",
            div { class: "settings-section-head",
                div {
                    h3 { class: "settings-section-title mono", "RUNNER MANAGEMENT" }
                    p { class: "setting-desc", "Manage Proton/Wine versions for your games" }
                }
                div { class: "settings-section-actions",
                    button {
                        class: "icon-btn",
                        title: "Rescan folders",
                        onclick: move |_| runners.restart(),
                        IconSvg { name: IconName::Refresh, size: 16 }
                    }
                    if proton_progress.is_none() {
                        button {
                            class: "btn btn-accent",
                            onclick: {
                                let store = store.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let s = store.clone();
                                    s.call(
                                        async move { core.download_proton().await },
                                        |s, _| s.toast(ToastKind::Success, "GE-Proton installed"),
                                    );
                                }
                            },
                            IconSvg { name: IconName::Add, size: 14 }
                            "GET GE-PROTON"
                        }
                    }
                }
            }

            if let Some(prog) = proton_progress {
                div { class: "runner-progress",
                    div { class: "runner-progress-head mono",
                        span { "Downloading {prog.version}…" }
                        span { "{(prog.progress * 100.0) as u32}%" }
                    }
                    div { class: "progress-bar",
                        div { class: "progress-fill", style: "width: {prog.progress * 100.0}%" }
                    }
                }
            }

            if runner_list.is_empty() {
                div { class: "settings-empty",
                    IconSvg { name: IconName::Settings, size: 48 }
                    p { "No runners found" }
                    span { "Install GE-Proton or add runners to the runners directory." }
                }
            }

            div { class: "runner-list",
                for runner in runner_list.iter() {
                    {
                        let is_default = settings.default_runner_id.as_deref() == Some(runner.as_str());
                        let runner_id = runner.clone();
                        rsx! {
                            div {
                                key: "{runner_id}",
                                class: if is_default { "runner-row default" } else { "runner-row" },
                                div { class: "runner-info",
                                    div { class: "runner-icon",
                                        IconSvg { name: IconName::Settings, size: 18 }
                                    }
                                    div {
                                        div { class: "runner-name",
                                            "{runner_id}"
                                            if is_default {
                                                span { class: "char-badge accent", "DEFAULT" }
                                            }
                                        }
                                        span { class: "setting-desc mono", "LOCAL STORAGE" }
                                    }
                                }
                                div { class: "runner-actions",
                                    if !is_default {
                                        button {
                                            class: "btn-link",
                                            onclick: {
                                                let store = store.clone();
                                                let rid = runner_id.clone();
                                                move |_| {
                                                    let mut s = store.clone();
                                                    let mut new_settings = (s.settings)();
                                                    new_settings.default_runner_id = Some(rid.clone());
                                                    s.save_settings(new_settings);
                                                }
                                            },
                                            "SET DEFAULT"
                                        }
                                    }
                                    button {
                                        class: "icon-btn icon-danger",
                                        title: "Remove",
                                        onclick: {
                                            let store = store.clone();
                                            let rid = runner_id.clone();
                                            move |_| {
                                                let core = store.core.clone();
                                                let rid = rid.clone();
                                                let s = store.clone();
                                                s.call(
                                                    async move { core.remove_runner(rid).await },
                                                    |s, _| s.toast(ToastKind::Success, "Runner removed"),
                                                );
                                            }
                                        },
                                        IconSvg { name: IconName::Trash, size: 16 }
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

// ─── Repos ───────────────────────────────────────────────────────

#[component]
fn ReposTab() -> Element {
    let store = use_context::<Store>();
    let config = (store.app_config)();
    let mut editing = use_signal(|| None::<String>);
    let mut local_val = use_signal(String::new);

    let repos = vec![
        (
            "common_loader_repo",
            "Common Loader Repository",
            "GitHub repo for the mod loader framework",
            config.common_loader_repo.clone(),
        ),
        (
            "proton_repo",
            "Proton Repository",
            "GitHub repo for GE-Proton builds",
            config.proton_repo.clone(),
        ),
    ];

    let edit_key = (editing)();

    rsx! {
        div { class: "settings-section",
            h3 { class: "settings-section-title mono", "REPOSITORY CONFIGURATION" }

            for (key, label, desc, value) in repos.iter() {
                div {
                    key: "{key}",
                    class: "setting-row setting-row-col",
                    div { class: "setting-row-top",
                        div { class: "setting-row-left",
                            div { class: "setting-icon",
                                IconSvg { name: IconName::Grid, size: 18 }
                            }
                            div {
                                div { class: "setting-label", "{label}" }
                                div { class: "setting-desc", "{desc}" }
                            }
                        }
                        if edit_key.as_deref() != Some(*key) {
                            button {
                                class: "btn-link",
                                onclick: {
                                    let key = key.to_string();
                                    let val = value.clone();
                                    move |_| {
                                        local_val.set(val.clone());
                                        editing.set(Some(key.clone()));
                                    }
                                },
                                "EDIT"
                            }
                        }
                    }
                    if edit_key.as_deref() == Some(*key) {
                        div { class: "path-editor",
                            input {
                                class: "input mono",
                                r#type: "text",
                                value: "{(local_val)()}",
                                oninput: move |evt| local_val.set(evt.value()),
                            }
                            div { class: "path-editor-actions",
                                button {
                                    class: "btn-link",
                                    onclick: move |_| editing.set(None),
                                    "CANCEL"
                                }
                                button {
                                    class: "btn btn-accent",
                                    onclick: {
                                        let store = store.clone();
                                        let key = key.to_string();
                                        move |_| {
                                            let val = (local_val)();
                                            let mut config = (store.app_config)();
                                            match key.as_str() {
                                                "common_loader_repo" => config.common_loader_repo = val,
                                                "proton_repo" => config.proton_repo = val,
                                                _ => {}
                                            }
                                            let core = store.core.clone();
                                            let config2 = config.clone();
                                            store.clone().call(
                                                async move { core.update_app_config(config2).await },
                                                |s, _| s.toast(ToastKind::Success, "Repository saved"),
                                            );
                                            editing.set(None);
                                        }
                                    },
                                    "SAVE"
                                }
                            }
                        }
                    } else {
                        div { class: "setting-value mono", "{value}" }
                    }
                }
            }
        }
    }
}

// ─── Shared toggle ───────────────────────────────────────────────

#[component]
fn ToggleRow(
    icon: IconName,
    label: String,
    description: String,
    checked: bool,
    ontoggle: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "setting-row",
            div { class: "setting-row-left",
                div { class: "setting-icon",
                    IconSvg { name: icon, size: 18 }
                }
                div {
                    div { class: "setting-label", "{label}" }
                    div { class: "setting-desc", "{description}" }
                }
            }
            button {
                class: if checked { "toggle-switch on" } else { "toggle-switch" },
                onclick: move |_| ontoggle.call(()),
                div { class: "toggle-knob" }
            }
        }
    }
}
