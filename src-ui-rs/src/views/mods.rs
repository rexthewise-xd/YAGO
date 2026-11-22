use dioxus::html::HasFileData;
use dioxus::prelude::*;
use storage::models::{ImportCandidate, ModRecord};
use uuid::Uuid;
use yago_app::ModUpdateInfo;

use crate::components::Confirm;
use crate::icons::{IconName, IconSvg};
use crate::modutil::{active_profile, is_nsfw, mod_preview_or_fallback, MoveDir};
use crate::picker::{pick_file, pick_folder};
use crate::state::{Store, ToastKind};
use crate::views::import_staging::ImportStaging;
use crate::views::mod_inspector::ModInspector;
use crate::views::GameBananaBrowserModal;

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    List,
    Grid,
}

#[component]
pub fn ModsView() -> Element {
    let store = use_context::<Store>();
    let mut search = use_signal(String::new);
    let mut selected_mod = use_signal(|| None::<String>);
    let mut view_mode = use_signal(|| ViewMode::List);
    let mut dev_mode = use_signal(|| false);
    let mut enabled_only = use_signal(|| false);
    let mut sort_name = use_signal(|| false);
    let mut importing = use_signal(|| false);
    let mut redeploying = use_signal(|| false);
    let mut staging = use_signal(|| None::<Vec<ImportCandidate>>);
    let mut confirm_delete = use_signal(|| None::<(String, String)>);
    let mut is_dragging_over = use_signal(|| false);
    let mut show_gb_browser = use_signal(|| false);
    let mut checking_updates = use_signal(|| false);
    let mut available_updates = use_signal(std::collections::HashMap::<String, ModUpdateInfo>::new);
    let mut updating_mod = use_signal(|| None::<String>);

    let Some(game) = store.selected_game() else {
        return rsx! {
            div { class: "overview-empty",
                IconSvg { name: IconName::Mods, size: 32 }
                h2 { "No title selected" }
                p { "Select a game to manage its mods." }
            }
        };
    };

    let game_id = game.id.clone();
    let install_path = game.install_path.to_string_lossy().to_string();
    let db = store.db(&game_id);
    let mods: Vec<ModRecord> = db
        .as_ref()
        .map(|d| d.mods.values().cloned().collect())
        .unwrap_or_default();
    let load_order: Vec<Uuid> = db
        .as_ref()
        .and_then(|d| active_profile(d, &game).map(|p| p.load_order.clone()))
        .unwrap_or_default();
    let enabled_ids: Vec<Uuid> = db
        .as_ref()
        .and_then(|d| active_profile(d, &game).map(|p| p.enabled_mod_ids.clone()))
        .unwrap_or_default();

    let settings = (store.settings)();
    let stream_safe = settings.stream_safe;
    let nsfw_hide = stream_safe && settings.nsfw_behavior == "hide";
    let term = (search)().to_lowercase();
    let only_enabled = (enabled_only)();
    let by_name = (sort_name)();
    let mode = (view_mode)();
    let dev = (dev_mode)();
    let is_importing = (importing)();
    let is_redeploying = (redeploying)();
    let selected = (selected_mod)();

    let mut list: Vec<(ModRecord, bool)> = mods
        .into_iter()
        .map(|m| {
            let en = enabled_ids.contains(&m.id);
            (m, en)
        })
        .filter(|(m, en)| {
            let matches = term.is_empty()
                || m.meta.name.to_lowercase().contains(&term)
                || m.meta.author.to_lowercase().contains(&term);
            let hidden = nsfw_hide && is_nsfw(&m.config.tags);
            matches && (!only_enabled || *en) && !hidden
        })
        .collect();

    if by_name {
        list.sort_by(|a, b| {
            a.0.meta
                .name
                .to_lowercase()
                .cmp(&b.0.meta.name.to_lowercase())
        });
    } else {
        list.sort_by_key(|(m, _)| load_order.iter().position(|id| *id == m.id).unwrap_or(9999));
    }

    let mod_count = list.len();
    let reorder_enabled = !by_name;
    let staging_snapshot = (staging)();
    let delete_snapshot = (confirm_delete)();

    let mut char_counts = std::collections::HashMap::<String, usize>::new();
    for (m, en) in list.iter() {
        if *en
            && m.compatibility.character != "Unknown"
            && m.compatibility.character != "Global/Other"
        {
            *char_counts
                .entry(m.compatibility.character.clone())
                .or_insert(0) += 1;
        }
    }

    rsx! {
        div {
            class: if (is_dragging_over)() { "mods-view drag-over" } else { "mods-view" },
            ondragover: move |evt| {
                evt.prevent_default();
                is_dragging_over.set(true);
            },
            ondragenter: move |evt| {
                evt.prevent_default();
                is_dragging_over.set(true);
            },
            ondragleave: move |_| {
                is_dragging_over.set(false);
            },
            ondrop: {
                let store = store.clone();
                let gid = game_id.clone();
                move |evt| {
                    evt.prevent_default();
                    is_dragging_over.set(false);
                    let files = evt.files();
                    if files.is_empty() {
                        return;
                    }
                    let core = store.core.clone();
                    let gid = gid.clone();
                    let mut s = store.clone();
                    spawn(async move {
                        importing.set(true);
                        let mut ok_count = 0usize;
                        let mut last_error = None;

                        for file in files {
                            let path = file.path();
                            let is_archive = path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .map(|ext| {
                                    let ext_lower = ext.to_lowercase();
                                    ext_lower == "zip" || ext_lower == "7z"
                                })
                                .unwrap_or(false);
                            let is_dir = path.is_dir();

                            if is_archive || is_dir {
                                match core
                                    .import_mod(gid.clone(), path.to_string_lossy().to_string())
                                    .await
                                {
                                    Ok(_) => ok_count += 1,
                                    Err(e) => {
                                        eprintln!("Import failed for {:?}: {}", path, e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                        }

                        if ok_count > 0 {
                            s.toast(ToastKind::Success, format!("{ok_count} mod(s) imported"));
                        } else if let Some(err) = last_error {
                            s.toast(ToastKind::Error, err);
                        }
                        importing.set(false);
                    });
                }
            },
            if (is_dragging_over)() {
                div { class: "mods-dropzone-overlay",
                    IconSvg { name: IconName::Folder, size: 48 }
                    p { "Drop mod archive (.zip, .7z) or folder to import" }
                }
            }
            div { class: "mods-header",
                div { class: "mods-title",
                    IconSvg { name: IconName::Mods, size: 20 }
                    h1 { "MOD MANAGER" }
                    span { class: "mods-count mono", "{mod_count}" }
                }
                div { class: "mods-header-actions",
                    div { class: "seg",
                        button {
                            class: if mode == ViewMode::List { "seg-btn active" } else { "seg-btn" },
                            title: "List view",
                            onclick: move |_| view_mode.set(ViewMode::List),
                            IconSvg { name: IconName::List, size: 16 }
                        }
                        button {
                            class: if mode == ViewMode::Grid { "seg-btn active" } else { "seg-btn" },
                            title: "Grid view",
                            onclick: move |_| view_mode.set(ViewMode::Grid),
                            IconSvg { name: IconName::Grid, size: 16 }
                        }
                    }
                    button {
                        class: if dev { "icon-btn icon-active" } else { "icon-btn" },
                        title: "Developer mode",
                        onclick: move |_| dev_mode.set(!dev),
                        IconSvg { name: IconName::Edit, size: 17 }
                    }
                    div { class: "mods-search",
                        IconSvg { name: IconName::Search, size: 15 }
                        input {
                            class: "input",
                            r#type: "text",
                            placeholder: "Search mods...",
                            value: "{(search)()}",
                            oninput: move |evt| search.set(evt.value()),
                        }
                    }
                    button {
                        class: "btn btn-accent",
                        disabled: is_importing,
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    let Some(path) = pick_file("Select Mod Archive (.zip, .7z)").await else { return };
                                    importing.set(true);
                                    match core.import_mod(gid, path.to_string_lossy().to_string()).await {
                                        Ok(_) => s.toast(ToastKind::Success, "Mod imported"),
                                        Err(e) => s.toast(ToastKind::Error, e),
                                    }
                                    importing.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Add, size: 15 }
                        if is_importing { "IMPORTING…" } else { "ADD MOD" }
                    }
                    button {
                        class: "btn",
                        disabled: is_importing,
                        title: "Import from folder",
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let mut s = store.clone();
                                let core = s.core.clone();
                                let gid = gid.clone();
                                spawn(async move {
                                    let Some(path) = pick_folder("Select Mod Folder").await else { return };
                                    importing.set(true);
                                    match core.scan_mod_directory(path.to_string_lossy().to_string(), gid).await {
                                        Ok(candidates) => {
                                            if candidates.is_empty() {
                                                importing.set(false);
                                            } else {
                                                staging.set(Some(candidates));
                                                importing.set(false);
                                            }
                                        }
                                        Err(e) => {
                                            s.toast(ToastKind::Error, e);
                                            importing.set(false);
                                        }
                                    }
                                });
                            }
                        },
                        IconSvg { name: IconName::Folder, size: 15 }
                        "IMPORT FOLDER"
                    }
                    button {
                        class: "btn",
                        disabled: (checking_updates)(),
                        title: "Check GameBanana for mod updates",
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    checking_updates.set(true);
                                    match core.check_mod_updates(gid).await {
                                        Ok(updates) => {
                                            let count = updates.len();
                                            let map: std::collections::HashMap<String, ModUpdateInfo> = updates
                                                .into_iter()
                                                .map(|u| (u.mod_uuid.clone(), u))
                                                .collect();
                                            available_updates.set(map);
                                            if count > 0 {
                                                s.toast(ToastKind::Info, format!("Found {count} mod update(s)"));
                                            } else {
                                                s.toast(ToastKind::Success, "All mods are up-to-date");
                                            }
                                        }
                                        Err(e) => s.toast(ToastKind::Error, e),
                                    }
                                    checking_updates.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Refresh, size: 15 }
                        if (checking_updates)() { "CHECKING…" } else { "CHECK UPDATES" }
                    }
                    button {
                        class: "btn btn-accent",
                        title: "Browse and install mods from GameBanana",
                        onclick: move |_| show_gb_browser.set(true),
                        IconSvg { name: IconName::Search, size: 15 }
                        "BROWSE GAMEBANANA"
                    }
                    button {
                        class: "btn",
                        disabled: is_redeploying,
                        title: "Reload mods in-game",
                        onclick: {
                            let store = store.clone();
                            let path = install_path.clone();
                            move |_| {
                                let core = store.core.clone();
                                let path = path.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    redeploying.set(true);
                                    match core.redeploy_mods(path).await {
                                        Ok(_) => s.toast(ToastKind::Success, "Mods redeployed"),
                                        Err(e) => s.toast(ToastKind::Error, e),
                                    }
                                    redeploying.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Refresh, size: 15 }
                        "RELOAD"
                    }
                }
            }

            div { class: "mods-body",
                if mode == ViewMode::List {
                    div { class: "mods-list-col",
                        div { class: "mods-sortbar",
                            div { class: "mods-sortbar-left",
                                button {
                                    class: if by_name { "icon-btn icon-active" } else { "icon-btn" },
                                    title: "Sort by name",
                                    onclick: move |_| sort_name.set(!by_name),
                                    IconSvg { name: IconName::List, size: 14 }
                                }
                                button {
                                    class: if only_enabled { "chip active" } else { "chip" },
                                    onclick: move |_| enabled_only.set(!only_enabled),
                                    "ACTIVE"
                                }
                            }
                            span { class: "mods-sort-hint mono", if by_name { "NAME" } else { "LOAD ORDER" } }
                        }
                        div { class: "mods-list",
                            if list.is_empty() {
                                div { class: "mods-empty",
                                    IconSvg { name: IconName::Mods, size: 40 }
                                    p { "No mods match. Import an archive or folder to begin." }
                                }
                            }
                            for (idx , (m, en)) in list.iter().enumerate() {
                                {
                                    let store = store.clone();
                                    let game_id = game_id.clone();
                                    let mid_str = m.id.to_string();
                                    let has_update = available_updates().contains_key(&mid_str);
                                    let is_updating = updating_mod().as_deref() == Some(&mid_str);
                                    let has_conflict = char_counts.get(&m.compatibility.character).copied().unwrap_or(0) > 1;
                                    rsx! {
                                        ModRow {
                                            key: "{m.id}",
                                            mod_record: m.clone(),
                                            enabled: *en,
                                            selected: selected.as_deref() == Some(m.id.to_string().as_str()),
                                            can_move_up: idx > 0,
                                            can_move_down: idx < list.len() - 1,
                                            reorder_enabled,
                                            dev_mode: dev,
                                            stream_safe,
                                            has_conflict,
                                            has_update,
                                            is_updating,
                                            onupdate: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                let mid = mid_str.clone();
                                                move |_| {
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let mid = mid.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        updating_mod.set(Some(mid.clone()));
                                                        match core.update_mod_from_gamebanana(gid, mid.clone()).await {
                                                            Ok(rec) => {
                                                                s.toast(ToastKind::Success, format!("Updated {}", rec.meta.name));
                                                                let mut map = available_updates();
                                                                map.remove(&mid);
                                                                available_updates.set(map);
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                        updating_mod.set(None);
                                                    });
                                                }
                                            },
                                            onselect: move |id: String| selected_mod.set(Some(id)),
                                            ontoggle: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                move |(id, val): (String, bool)| {
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let s = store.clone();
                                                    s.call(async move { core.toggle_mod(gid, id, val).await }, |_, _| {});
                                                }
                                            },
                                            onmove: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                move |(id, dir): (String, MoveDir)| do_move_mod(store.clone(), gid.clone(), id, dir)
                                            },
                                            ondelete: {
                                                let store = store.clone();
                                                let gid = game_id.clone();
                                                move |id: String| {
                                                    let name = store.db(&gid)
                                                        .and_then(|d| Uuid::parse_str(&id).ok().and_then(|u| d.mods.get(&u).map(|m| m.meta.name.clone())))
                                                        .unwrap_or_default();
                                                    confirm_delete.set(Some((id, name)));
                                                }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "mods-inspector-col",
                        ModInspector {
                            selected_mod,
                            game_id: game_id.clone(),
                            dev_mode: dev,
                            is_sorted: by_name,
                            onmove: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |(id, dir): (String, MoveDir)| do_move_mod(store.clone(), gid.clone(), id, dir)
                            },
                        }
                    }
                } else {
                    div { class: "mods-grid",
                        for (m, en) in list.iter() {
                            {
                                let store = store.clone();
                                let game_id = game_id.clone();
                                let mid_str = m.id.to_string();
                                let has_update = available_updates().contains_key(&mid_str);
                                rsx! {
                                    GridModCard {
                                        key: "{m.id}",
                                        mod_record: m.clone(),
                                        enabled: *en,
                                        selected: selected.as_deref() == Some(m.id.to_string().as_str()),
                                        stream_safe,
                                        has_update,
                                        onselect: move |id: String| selected_mod.set(Some(id)),
                                        ontoggle: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            move |(id, val): (String, bool)| {
                                                let core = store.core.clone();
                                                let gid = gid.clone();
                                                let s = store.clone();
                                                s.call(async move { core.toggle_mod(gid, id, val).await }, |_, _| {});
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if (show_gb_browser)() {
                GameBananaBrowserModal {
                    game_id: game_id.clone(),
                    onclose: move |_| show_gb_browser.set(false),
                }
            }

            if let Some(candidates) = staging_snapshot {
                ImportStaging {
                    candidates: candidates.clone(),
                    onconfirm: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        move |items: Vec<ImportCandidate>| {
                            staging.set(None);
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let mut s = store.clone();
                            spawn(async move {
                                importing.set(true);
                                let mut ok = 0usize;
                                for c in &items {
                                    if c.initial_state {
                                        match core.import_mod_with_name(
                                            gid.clone(),
                                            c.original_path.to_string_lossy().to_string(),
                                            Some(c.suggested_name.clone()),
                                        ).await {
                                            Ok(_) => ok += 1,
                                            Err(e) => s.toast(ToastKind::Error, e),
                                        }
                                    }
                                }
                                if ok > 0 {
                                    s.toast(ToastKind::Success, format!("{ok} mod(s) imported"));
                                }
                                importing.set(false);
                            });
                        }
                    },
                    onclose: move |_| staging.set(None),
                }
            }

            if let Some((del_id, del_name)) = delete_snapshot {
                Confirm {
                    title: "Delete Mod".to_string(),
                    message: format!("Move \"{del_name}\" to Recycle Bin / Trash? (Non-destructive)"),
                    confirm_label: "Move to Trash".to_string(),
                    danger: true,
                    onconfirm: {
                        let store = store.clone();
                        move |_| {
                            let core = store.core.clone();
                            let mid = del_id.clone();
                            let s = store.clone();
                            s.call(
                                async move { core.delete_mod(mid).await },
                                |s, _| s.toast(ToastKind::Success, "Mod moved to Recycle Bin / Trash"),
                            );
                            confirm_delete.set(None);
                        }
                    },
                    oncancel: move |_| confirm_delete.set(None),
                }
            }
        }
    }
}

fn do_move_mod(store: Store, game_id: String, mod_id: String, dir: MoveDir) {
    let Some(db) = store.db(&game_id) else { return };
    let Some(game) = store.game(&game_id) else {
        return;
    };
    let Some(profile) = active_profile(&db, &game) else {
        return;
    };
    let mut order = profile.load_order.clone();
    let Some(uid) = Uuid::parse_str(&mod_id).ok() else {
        return;
    };
    let Some(idx) = order.iter().position(|id| *id == uid) else {
        return;
    };

    match dir {
        MoveDir::Up if idx > 0 => {
            order.swap(idx, idx - 1);
        }
        MoveDir::Down if idx < order.len() - 1 => {
            order.swap(idx, idx + 1);
        }
        MoveDir::Top if idx > 0 => {
            let item = order.remove(idx);
            order.insert(0, item);
        }
        MoveDir::Bottom if idx < order.len() - 1 => {
            let item = order.remove(idx);
            order.push(item);
        }
        _ => return,
    }

    let core = store.core.clone();
    let s = store.clone();
    s.call(
        async move { core.set_load_order(game_id, order).await },
        |_, _| {},
    );
}

// ─── ModRow ──────────────────────────────────────────────────────

#[component]
fn ModRow(
    mod_record: ModRecord,
    enabled: bool,
    selected: bool,
    can_move_up: bool,
    can_move_down: bool,
    reorder_enabled: bool,
    dev_mode: bool,
    stream_safe: bool,
    #[props(default)] has_conflict: bool,
    #[props(default)] has_update: bool,
    #[props(default)] is_updating: bool,
    #[props(default)] onupdate: Option<EventHandler<()>>,
    onselect: EventHandler<String>,
    ontoggle: EventHandler<(String, bool)>,
    onmove: EventHandler<(String, MoveDir)>,
    ondelete: EventHandler<String>,
) -> Element {
    let store = use_context::<Store>();
    let m = &mod_record;
    let mid = m.id.to_string();
    let nsfw = is_nsfw(&m.config.tags);
    let blur = nsfw && stream_safe;
    let preview = mod_preview_or_fallback(&store, m);
    let character = m.compatibility.character.clone();
    let size = m.size.clone();

    rsx! {
        div {
            class: {
                let mut c = String::from("mod-row");
                if selected { c.push_str(" selected"); }
                if enabled { c.push_str(" enabled"); }
                c
            },
            onclick: {
                let mid = mid.clone();
                move |_| onselect.call(mid.clone())
            },

            div { class: "mod-row-toggle",
                button {
                    class: if enabled { "switch on" } else { "switch" },
                    onclick: {
                        let mid = mid.clone();
                        let en = enabled;
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            ontoggle.call((mid.clone(), !en));
                        }
                    },
                    span { class: "switch-knob" }
                }
            }

            div { class: "mod-row-thumb",
                img { class: if blur { "blurred" } else { "" }, src: "{preview}", alt: "" }
            }

            div { class: "mod-row-info",
                span { class: if blur { "mod-row-name blurred" } else { "mod-row-name" }, "{m.meta.name}" }
                div { class: "mod-row-meta mono",
                    span { "{m.meta.author}" }
                    span { "v{m.meta.version}" }
                    span { "{character}" }
                    span { "{size}" }
                    if has_conflict && enabled {
                        span { class: "char-badge accent", title: "Multiple mods active for this character — resolved by load order", "CYCLE" }
                    }
                    if has_update {
                        span { class: "update-badge", "UPDATE AVAILABLE" }
                    }
                }
            }

            if reorder_enabled {
                div { class: "mod-row-reorder",
                    button {
                        class: "icon-btn",
                        disabled: !can_move_up,
                        onclick: {
                            let mid = mid.clone();
                            move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                onmove.call((mid.clone(), MoveDir::Up));
                            }
                        },
                        IconSvg { name: IconName::ChevronDown, size: 13 }
                    }
                    button {
                        class: "icon-btn",
                        disabled: !can_move_down,
                        onclick: {
                            let mid = mid.clone();
                            move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                onmove.call((mid.clone(), MoveDir::Down));
                            }
                        },
                        IconSvg { name: IconName::ChevronRight, size: 13 }
                    }
                }
            }

            div { class: "mod-row-actions",
                if has_update {
                    if let Some(onup) = onupdate {
                        button {
                            class: "btn btn-sm btn-accent",
                            disabled: is_updating,
                            onclick: move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                onup.call(());
                            },
                            if is_updating { "UPDATING…" } else { "UPDATE" }
                        }
                    }
                }
                if dev_mode {
                    button {
                        class: "icon-btn icon-danger",
                        title: "Uninstall",
                        onclick: {
                            let mid = mid.clone();
                            move |evt: Event<MouseData>| {
                                evt.stop_propagation();
                                ondelete.call(mid.clone());
                            }
                        },
                        IconSvg { name: IconName::Trash, size: 15 }
                    }
                }
            }
        }
    }
}

// ─── GridModCard ─────────────────────────────────────────────────

#[component]
fn GridModCard(
    mod_record: ModRecord,
    enabled: bool,
    selected: bool,
    stream_safe: bool,
    #[props(default)] has_update: bool,
    onselect: EventHandler<String>,
    ontoggle: EventHandler<(String, bool)>,
) -> Element {
    let store = use_context::<Store>();
    let m = &mod_record;
    let mid = m.id.to_string();
    let nsfw = is_nsfw(&m.config.tags);
    let blur = nsfw && stream_safe;
    let preview = mod_preview_or_fallback(&store, m);

    rsx! {
        div {
            class: {
                let mut c = String::from("grid-card");
                if selected { c.push_str(" selected"); }
                if !enabled { c.push_str(" disabled"); }
                c
            },
            onclick: {
                let mid = mid.clone();
                move |_| onselect.call(mid.clone())
            },
            div { class: "grid-card-img",
                img { class: if blur { "blurred" } else { "" }, src: "{preview}", alt: "" }
                div { class: "grid-card-shade" }
                if has_update {
                    span {
                        style: "position: absolute; top: 6px; left: 6px; z-index: 5;",
                        class: "update-badge",
                        "UPDATE"
                    }
                }
                button {
                    class: if enabled { "switch on grid-switch" } else { "switch grid-switch" },
                    onclick: {
                        let mid = mid.clone();
                        let en = enabled;
                        move |evt: Event<MouseData>| {
                            evt.stop_propagation();
                            ontoggle.call((mid.clone(), !en));
                        }
                    },
                    span { class: "switch-knob" }
                }
            }
            div { class: "grid-card-body",
                span { class: if blur { "grid-card-name blurred" } else { "grid-card-name" }, "{m.meta.name}" }
                span { class: "grid-card-author mono", "{m.meta.author}" }
            }
        }
    }
}
