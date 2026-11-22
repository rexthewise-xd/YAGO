use dioxus::prelude::*;
use uuid::Uuid;
use yago_app::{FileNode, FixPreview};

use crate::components::Confirm;
use crate::icons::{IconName, IconSvg};
use crate::modutil::{is_nsfw, mod_preview_url, tag_class, MoveDir};
use crate::state::{Store, ToastKind};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Info,
    Files,
    Editor,
}

fn is_text_file(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(
        ext.as_str(),
        "json" | "ini" | "txt" | "cfg" | "xml" | "lua" | "py" | "js" | "ts" | "md"
    )
}

fn file_icon(name: &str) -> IconName {
    let lower = name.to_lowercase();
    if lower.ends_with(".png") || lower.ends_with(".dds") || lower.ends_with(".jpg") {
        IconName::Image
    } else if lower.ends_with(".ini") || lower.ends_with(".cfg") {
        IconName::Settings
    } else {
        IconName::File
    }
}

#[component]
pub fn ModInspector(
    selected_mod: Signal<Option<String>>,
    game_id: String,
    dev_mode: bool,
    is_sorted: bool,
    onmove: EventHandler<(String, MoveDir)>,
) -> Element {
    let store = use_context::<Store>();
    let mut tab = use_signal(|| Tab::Info);
    let closed = use_signal(Vec::<String>::new);
    let mut active_file = use_signal(|| None::<String>);
    let mut content = use_signal(String::new);
    let mut adding_tag = use_signal(|| false);
    let mut new_tag = use_signal(String::new);
    let mut confirm_delete = use_signal(|| false);
    let mut previewing_fix = use_signal(|| false);
    let mut applying_fix = use_signal(|| false);
    let mut fix_preview = use_signal(|| None::<FixPreview>);
    let mut backup_refresh = use_signal(|| 0usize);

    let sel = (selected_mod)();
    let record = sel.as_ref().and_then(|id| {
        let db = store.db(&game_id)?;
        let uuid = Uuid::parse_str(id).ok()?;
        db.mods.get(&uuid).cloned()
    });

    let core = store.core.clone();
    let files = use_resource(move || {
        let core = core.clone();
        let id = (selected_mod)();
        async move {
            match id {
                Some(id) => core.get_mod_files(id).await.unwrap_or_default(),
                None => Vec::new(),
            }
        }
    });

    use_effect(move || {
        let _ = (selected_mod)();
        active_file.set(None);
        content.set(String::new());
        fix_preview.set(None);
    });

    let Some(record) = record else {
        return rsx! {
            div { class: "inspector-empty",
                IconSvg { name: IconName::File, size: 48 }
                h3 { "No Mod Selected" }
                p { "Select a mod to inspect." }
            }
        };
    };

    let mod_id = record.id.to_string();
    let current_tab = (tab)();
    let tree = (files)().unwrap_or_default();
    let active = (active_file)();
    let preview = mod_preview_url(&store, &record);
    let nsfw = is_nsfw(&record.config.tags);
    let stream_safe = (store.settings)().stream_safe;
    let blur = nsfw && stream_safe;
    let description = record
        .meta
        .description
        .clone()
        .unwrap_or_else(|| "No description available".to_string());
    let updated = record.added_at.format("%Y-%m-%d").to_string();

    let mid_top = mod_id.clone();
    let mid_up = mod_id.clone();
    let mid_down = mod_id.clone();
    let mid_bottom = mod_id.clone();
    let validate_id = mod_id.clone();
    let validate_store = store.clone();
    let tags_gid = game_id.clone();
    let tags_mid = mod_id.clone();
    let existing_tags = record.config.tags.clone();
    let tag_store = store.clone();
    let add_gid = game_id.clone();
    let add_mid = mod_id.clone();
    let add_tags = record.config.tags.clone();
    let add_store = store.clone();
    let confirm_mid = mod_id.clone();
    let confirm_name = record.meta.name.clone();
    let confirm_store = store.clone();

    let core_backup = store.core.clone();
    let mod_id_backup = mod_id.clone();
    let b_trigger = (backup_refresh)();
    let has_backup_res = use_resource(move || {
        let core = core_backup.clone();
        let mid = mod_id_backup.clone();
        let _ = b_trigger;
        async move { core.has_mod_fix_backup(mid).await.unwrap_or(false) }
    });
    let has_backup = (has_backup_res)().unwrap_or(false);

    rsx! {
        div { class: "inspector",
            div { class: "inspector-head",
                div { class: "inspector-tabs",
                    button {
                        class: if current_tab == Tab::Info { "itab active" } else { "itab" },
                        onclick: move |_| tab.set(Tab::Info),
                        IconSvg { name: IconName::Info, size: 13 }
                        "INFO"
                    }
                    button {
                        class: if current_tab == Tab::Files { "itab active" } else { "itab" },
                        onclick: move |_| tab.set(Tab::Files),
                        IconSvg { name: IconName::Folder, size: 13 }
                        "FILES"
                    }
                    if dev_mode {
                        button {
                            class: if current_tab == Tab::Editor { "itab active" } else { "itab" },
                            onclick: move |_| tab.set(Tab::Editor),
                            IconSvg { name: IconName::Edit, size: 13 }
                            "EDITOR"
                        }
                    }
                }
                if is_sorted {
                    span { class: "inspector-sorted mono", IconSvg { name: IconName::List, size: 13 } "SORTED" }
                } else {
                    div { class: "inspector-reorder",
                        button { class: "icon-btn", title: "Move to top", onclick: move |_| onmove.call((mid_top.clone(), MoveDir::Top)), IconSvg { name: IconName::ChevronDown, size: 15 } }
                        button { class: "icon-btn", title: "Move up", onclick: move |_| onmove.call((mid_up.clone(), MoveDir::Up)), IconSvg { name: IconName::ChevronDown, size: 15 } }
                        button { class: "icon-btn", title: "Move down", onclick: move |_| onmove.call((mid_down.clone(), MoveDir::Down)), IconSvg { name: IconName::ChevronRight, size: 15 } }
                        button { class: "icon-btn", title: "Move to bottom", onclick: move |_| onmove.call((mid_bottom.clone(), MoveDir::Bottom)), IconSvg { name: IconName::ChevronRight, size: 15 } }
                    }
                }
            }

            div { class: "inspector-body",
                if current_tab == Tab::Info {
                    div { class: "inspector-info",
                        div { class: "info-hero",
                            if let Some(p) = preview {
                                img { class: if blur { "info-hero-img blurred" } else { "info-hero-img" }, src: "{p}", alt: "" }
                            } else {
                                div { class: "info-hero-placeholder", IconSvg { name: IconName::Image, size: 48 } }
                            }
                            div { class: "info-hero-shade" }
                            div { class: "info-hero-text",
                                h2 { class: if blur { "blurred" } else { "" }, "{record.meta.name}" }
                                div { class: "info-hero-meta mono",
                                    span { class: "info-avatar", "{record.meta.author.chars().next().unwrap_or('?').to_uppercase()}" }
                                    span { "{record.meta.author}" }
                                    span { class: "info-version", "v{record.meta.version}" }
                                }
                            }
                        }

                        p { class: if blur { "info-desc blurred" } else { "info-desc" }, "{description}" }

                        div { class: "info-stats",
                            div { class: "info-stat",
                                IconSvg { name: IconName::File, size: 18 }
                                div { div { class: "info-stat-label mono", "SIZE" } div { class: "info-stat-val mono", "{record.size}" } }
                            }
                            div { class: "info-stat",
                                IconSvg { name: IconName::Refresh, size: 18 }
                                div { div { class: "info-stat-label mono", "ADDED" } div { class: "info-stat-val mono", "{updated}" } }
                            }
                        }

                        if let Some(url) = record.meta.url.clone() {
                            div { class: "info-url",
                                IconSvg { name: IconName::Info, size: 16 }
                                div { div { class: "info-url-label mono", "MOD HOMEPAGE" } div { class: "info-url-val", "{url}" } }
                            }
                        }

                        div { class: "info-tags",
                            div { class: "field-label mono", IconSvg { name: IconName::Tag, size: 13 } "TAGS" }
                            div { class: "tag-wrap",
                                for t in record.config.tags.iter() {
                                    span { key: "{t}", class: tag_class(t),
                                        "{t}"
                                        button {
                                            class: "tag-x",
                                            onclick: {
                                                let removed = t.clone();
                                                let tags = existing_tags.clone();
                                                let gid = tags_gid.clone();
                                                let mid = tags_mid.clone();
                                                let s = tag_store.clone();
                                                move |_| {
                                                    let next: Vec<String> = tags.iter().filter(|x| *x != &removed).cloned().collect();
                                                    let core = s.core.clone();
                                                    let gid = gid.clone();
                                                    let mid = mid.clone();
                                                    s.call(async move { core.update_mod_tags(gid, mid, next).await }, |_, _| {});
                                                }
                                            },
                                            IconSvg { name: IconName::Close, size: 9 }
                                        }
                                    }
                                }
                                if (adding_tag)() {
                                    input {
                                        class: "input tag-input",
                                        r#type: "text",
                                        autofocus: true,
                                        value: "{(new_tag)()}",
                                        oninput: move |evt| new_tag.set(evt.value()),
                                        onblur: move |_| adding_tag.set(false),
                                        onkeydown: move |evt| {
                                            if evt.key() == Key::Enter {
                                                let val = (new_tag)().trim().to_string();
                                                if !val.is_empty() {
                                                    let mut next = add_tags.clone();
                                                    next.push(val);
                                                    let core = add_store.core.clone();
                                                    let gid = add_gid.clone();
                                                    let mid = add_mid.clone();
                                                    add_store.call(async move { core.update_mod_tags(gid, mid, next).await }, |_, _| {});
                                                }
                                                new_tag.set(String::new());
                                                adding_tag.set(false);
                                            }
                                        },
                                    }
                                } else {
                                    button { class: "tag-add", onclick: move |_| adding_tag.set(true), IconSvg { name: IconName::Add, size: 11 } "ADD TAG" }
                                }
                            }
                        }

                        if !record.config.keybinds.is_empty() {
                            div { class: "info-tags",
                                div { class: "field-label mono", IconSvg { name: IconName::Settings, size: 13 } "KEYBINDS & CONTROLS" }
                                div { class: "keybinds-list",
                                    for (kname, kb) in record.config.keybinds.iter() {
                                        div { key: "{kname}", class: "keybind-row mono",
                                            span { class: "keybind-badge", "[{kb.key}]" }
                                            span { class: "keybind-label", "{kb.label}" }
                                            if !kb.values.is_empty() {
                                                span { class: "keybind-values", "({kb.values.join(\", \")})" }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        div { class: "info-tools",
                            if record.config.mod_type.as_deref() == Some("character") {
                                div { class: "info-tool-box",
                                    div { class: "info-tool-title mono", IconSvg { name: IconName::Deploy, size: 13 } "CHARACTER INTEGRATION" }
                                    p { "This character skin is isolated via the YAGO Router." }
                                }
                            } else {
                                div { class: "info-tool-box",
                                    div { class: "info-tool-title mono", IconSvg { name: IconName::Info, size: 13 } "GLOBAL UI PROTOCOL" }
                                    p { "Persistent UI enhancement. Bypasses character gates." }
                                }
                            }

                            div { class: "info-tool-box",
                                div { class: "info-tool-title mono", IconSvg { name: IconName::Settings, size: 13 } "PATCH FIXER ENGINE" }
                                p { "Apply data-driven model shader patches (ORFix, FaceFix, slot remaps & hash migrations)." }

                                div { style: "display: flex; gap: 8px; margin-top: 8px; flex-wrap: wrap;",
                                    button {
                                        class: "btn btn-sm",
                                        disabled: (previewing_fix)() || (applying_fix)(),
                                        onclick: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            move |_| {
                                                let core = store.core.clone();
                                                let gid = gid.clone();
                                                let mid = mid.clone();
                                                let mut s = store.clone();
                                                spawn(async move {
                                                    previewing_fix.set(true);
                                                    match core.preview_mod_fix(gid, mid).await {
                                                        Ok(prev) => {
                                                            if !prev.has_changes() {
                                                                s.toast(ToastKind::Info, "No patch fixes needed (mod is already up-to-date)");
                                                            }
                                                            fix_preview.set(Some(prev));
                                                        }
                                                        Err(e) => s.toast(ToastKind::Error, e),
                                                    }
                                                    previewing_fix.set(false);
                                                });
                                            }
                                        },
                                        IconSvg { name: IconName::Search, size: 13 }
                                        if (previewing_fix)() { "ANALYZING…" } else { "PREVIEW FIX" }
                                    }

                                    if has_backup {
                                        button {
                                            class: "btn btn-sm",
                                            disabled: (applying_fix)(),
                                            onclick: {
                                                let store = store.clone();
                                                let mid = mod_id.clone();
                                                move |_| {
                                                    let core = store.core.clone();
                                                    let mid = mid.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        applying_fix.set(true);
                                                        match core.rollback_mod_fix(mid).await {
                                                            Ok(count) => {
                                                                s.toast(ToastKind::Success, format!("Restored {count} file(s) from backup"));
                                                                fix_preview.set(None);
                                                                backup_refresh.set(backup_refresh() + 1);
                                                            }
                                                            Err(e) => s.toast(ToastKind::Error, e),
                                                        }
                                                        applying_fix.set(false);
                                                    });
                                                }
                                            },
                                            IconSvg { name: IconName::Refresh, size: 13 }
                                            "ROLLBACK LAST FIX"
                                        }
                                    }
                                }

                                if let Some(prev) = (fix_preview)() {
                                    div {
                                        style: "margin-top: 10px; padding: 10px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px; display: flex; flex-direction: column; gap: 8px;",
                                        div { style: "display: flex; gap: 6px; flex-wrap: wrap; font-size: 11px;",
                                            span { class: "tag", "Hash fixes: {prev.total_hash_fixes}" }
                                            span { class: "tag", "Slot remaps: {prev.total_slot_fixes}" }
                                            span { class: "tag", "ORFix: {prev.total_orfix_fixes}" }
                                            if !prev.skipped_noapplier.is_empty() {
                                                span { class: "tag tag-outline", style: "color: var(--warn);",
                                                    "NOAPPLIER: {prev.skipped_noapplier.len()} skipped"
                                                }
                                            }
                                        }

                                        if prev.file_diffs.is_empty() {
                                            p { style: "font-size: 11px; color: var(--muted); margin: 4px 0;",
                                                "No file modifications required."
                                            }
                                        } else {
                                            div {
                                                style: "max-height: 180px; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; margin: 4px 0;",
                                                for diff_item in prev.file_diffs.iter() {
                                                    div {
                                                        style: "background: var(--bg); border: 1px solid var(--border); padding: 6px; border-radius: 2px;",
                                                        div { class: "mono", style: "font-size: 10px; font-weight: 600; color: var(--accent); margin-bottom: 4px;",
                                                            "{diff_item.relative_path} ({diff_item.changes_count} changes)"
                                                        }
                                                        pre {
                                                            class: "mono",
                                                            style: "margin: 0; font-size: 10px; line-height: 1.4; max-height: 100px; overflow-x: auto; white-space: pre-wrap; color: var(--text-sec);",
                                                            "{diff_item.diff}"
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        div { style: "display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px;",
                                            button {
                                                class: "btn btn-sm",
                                                onclick: move |_| fix_preview.set(None),
                                                "DISMISS"
                                            }
                                            if prev.has_changes() {
                                                button {
                                                    class: "btn btn-sm btn-accent",
                                                    disabled: (applying_fix)(),
                                                    onclick: {
                                                        let store = store.clone();
                                                        let gid = game_id.clone();
                                                        let mid = mod_id.clone();
                                                        move |_| {
                                                            let core = store.core.clone();
                                                            let gid = gid.clone();
                                                            let mid = mid.clone();
                                                            let mut s = store.clone();
                                                            spawn(async move {
                                                                applying_fix.set(true);
                                                                match core.apply_mod_fix(gid, mid).await {
                                                                    Ok(rep) => {
                                                                        s.toast(ToastKind::Success, format!("Patched {} file(s) successfully (backup saved)", rep.files_modified));
                                                                        fix_preview.set(None);
                                                                        backup_refresh.set(backup_refresh() + 1);
                                                                    }
                                                                    Err(e) => s.toast(ToastKind::Error, e),
                                                                }
                                                                applying_fix.set(false);
                                                            });
                                                        }
                                                    },
                                                    if (applying_fix)() { "APPLYING…" } else { "APPLY FIX" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            button {
                                class: "btn btn-block",
                                onclick: {
                                    let open_store = store.clone();
                                    let mpath = record.path.to_string_lossy().to_string();
                                    move |_| {
                                        let core = open_store.core.clone();
                                        let p = mpath.clone();
                                        open_store.call(async move { core.open_path(p).await }, |_, _| {});
                                    }
                                },
                                IconSvg { name: IconName::Folder, size: 15 }
                                "OPEN MOD FOLDER"
                            }
                            button {
                                class: "btn btn-block",
                                onclick: move |_| {
                                    let core = validate_store.core.clone();
                                    let mid = validate_id.clone();
                                    validate_store.call(
                                        async move { core.validate_mod(mid).await },
                                        |s, ok| s.toast(if ok { ToastKind::Success } else { ToastKind::Error }, if ok { "Mod logic validated" } else { "Validation failed" }),
                                    );
                                },
                                IconSvg { name: IconName::Deploy, size: 15 }
                                "VALIDATE LOGIC"
                            }
                            button {
                                class: "btn btn-danger btn-block",
                                onclick: move |_| confirm_delete.set(true),
                                IconSvg { name: IconName::Trash, size: 15 }
                                "UNINSTALL"
                            }
                        }
                    }
                }

                if current_tab == Tab::Files {
                    div { class: "inspector-files",
                        if tree.is_empty() {
                            div { class: "files-empty", IconSvg { name: IconName::Folder, size: 40 } p { "No files found." } }
                        } else {
                            div { class: "file-tree",
                                for node in tree.iter() {
                                    FileTreeNode {
                                        key: "{node.id}",
                                        node: node.clone(),
                                        level: 0,
                                        closed,
                                        active_file,
                                        onfile: {
                                            let store = store.clone();
                                            let mod_id = mod_id.clone();
                                            move |node: FileNode| {
                                                active_file.set(Some(node.id.clone()));
                                                if is_text_file(&node.name) {
                                                    let core = store.core.clone();
                                                    let mid = mod_id.clone();
                                                    let fid = node.id.clone();
                                                    let mut content = content;
                                                    let mut tab = tab;
                                                    spawn(async move {
                                                        if let Ok(c) = core.read_mod_file(mid, fid).await {
                                                            content.set(c);
                                                            tab.set(Tab::Editor);
                                                        }
                                                    });
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }

                if current_tab == Tab::Editor && dev_mode {
                    div { class: "inspector-editor",
                        if let Some(fid) = active.clone() {
                            div { class: "editor-bar",
                                span { class: "editor-file mono", IconSvg { name: IconName::File, size: 13 } "{fid}" }
                                button {
                                    class: "icon-btn",
                                    title: "Reload",
                                    onclick: {
                                        let fid = fid.clone();
                                        let mid = mod_id.clone();
                                        let core = store.core.clone();
                                        move |_| {
                                            let core = core.clone();
                                            let mid = mid.clone();
                                            let fid = fid.clone();
                                            let mut content = content;
                                            spawn(async move {
                                                if let Ok(c) = core.read_mod_file(mid, fid).await {
                                                    content.set(c);
                                                }
                                            });
                                        }
                                    },
                                    IconSvg { name: IconName::Refresh, size: 15 }
                                }
                                button {
                                    class: "icon-btn icon-accent",
                                    title: "Save",
                                    onclick: {
                                        let fid = fid.clone();
                                        let mid = mod_id.clone();
                                        let s = store.clone();
                                        move |_| {
                                            let core = s.core.clone();
                                            let mid = mid.clone();
                                            let fid = fid.clone();
                                            let body = (content)();
                                            s.call(async move { core.write_mod_file(mid, fid, body).await }, |s, _| s.toast(ToastKind::Success, "File saved"));
                                        }
                                    },
                                    IconSvg { name: IconName::Save, size: 15 }
                                }
                            }
                            textarea {
                                class: "editor-area mono",
                                spellcheck: false,
                                value: "{(content)()}",
                                oninput: move |evt| content.set(evt.value()),
                            }
                        } else {
                            div { class: "editor-empty",
                                IconSvg { name: IconName::Edit, size: 48 }
                                p { "No File Selected" }
                                span { "Select a text-based file from the explorer to edit." }
                            }
                        }
                    }
                }
            }

            if (confirm_delete)() {
                Confirm {
                    title: "Delete Mod".to_string(),
                    message: format!("Move \"{confirm_name}\" to Recycle Bin / Trash? (Non-destructive)"),
                    confirm_label: "Move to Trash".to_string(),
                    danger: true,
                    onconfirm: move |_| {
                        let core = confirm_store.core.clone();
                        let mid = confirm_mid.clone();
                        confirm_store.call(
                            async move { core.delete_mod(mid).await },
                            move |s, _| {
                                s.toast(ToastKind::Success, "Mod moved to Recycle Bin / Trash");
                            },
                        );
                        selected_mod.set(None);
                        confirm_delete.set(false);
                    },
                    oncancel: move |_| confirm_delete.set(false),
                }
            }
        }
    }
}

#[component]
fn FileTreeNode(
    node: FileNode,
    level: usize,
    closed: Signal<Vec<String>>,
    active_file: Signal<Option<String>>,
    onfile: EventHandler<FileNode>,
) -> Element {
    let is_folder = node.kind == "folder";
    let is_open = !closed.read().contains(&node.id);
    let is_active = (active_file)().as_deref() == Some(node.id.as_str());
    let pad = level * 16 + 12;
    let children = node.children.clone().unwrap_or_default();
    let toggle_id = node.id.clone();
    let click_node = node.clone();
    let icon = if is_folder {
        IconName::Folder
    } else {
        file_icon(&node.name)
    };

    rsx! {
        div { class: "tree-node",
            div {
                class: if is_active { "tree-row active" } else { "tree-row" },
                style: "padding-left: {pad}px",
                onclick: move |_| {
                    if is_folder {
                        let mut c = closed.write();
                        if let Some(pos) = c.iter().position(|x| x == &toggle_id) {
                            c.remove(pos);
                        } else {
                            c.push(toggle_id.clone());
                        }
                    } else {
                        onfile.call(click_node.clone());
                    }
                },
                span { class: "tree-chevron",
                    if is_folder {
                        IconSvg { name: if is_open { IconName::ChevronDown } else { IconName::ChevronRight }, size: 12 }
                    }
                }
                IconSvg { name: icon, size: 15 }
                span { class: "tree-name", "{node.name}" }
                if let Some(size) = node.size.clone() {
                    span { class: "tree-size mono", "{size}" }
                }
            }
            if is_folder && is_open {
                for child in children.iter() {
                    FileTreeNode { key: "{child.id}", node: child.clone(), level: level + 1, closed, active_file, onfile }
                }
            }
        }
    }
}
