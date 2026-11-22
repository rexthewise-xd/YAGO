use dioxus::prelude::*;
use uuid::Uuid;
use yago_app::{FileNode, FixPreview, IniAnchor, MeshPreviewData};

use crate::components::Confirm;
use crate::icons::{IconName, IconSvg};
use crate::modutil::{is_nsfw, mod_preview_url, tag_class, MoveDir};
use crate::picker::{pick_folder, pick_save_file};
use crate::state::{Store, ToastKind};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Info,
    Files,
    Editor,
    Mesh,
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
    let mut exporting_clean = use_signal(|| false);
    let mut syncing_dvt = use_signal(|| false);
    let mut fix_preview = use_signal(|| None::<FixPreview>);
    let mut backup_refresh = use_signal(|| 0usize);
    let mut capturing_keybind = use_signal(|| None::<String>);
    let mut creating_preset = use_signal(|| false);
    let mut new_preset_name = use_signal(String::new);
    let mut preset_refresh = use_signal(|| 0usize);
    let mut new_anchor_sec = use_signal(String::new);
    let mut new_anchor_key = use_signal(String::new);
    let mut new_anchor_val = use_signal(String::new);
    let mut adding_anchor = use_signal(|| false);

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

    let core_presets = store.core.clone();
    let gid_presets = game_id.clone();
    let mid_presets = mod_id.clone();
    let p_trigger = (preset_refresh)();
    let presets_res = use_resource(move || {
        let core = core_presets.clone();
        let gid = gid_presets.clone();
        let mid = mid_presets.clone();
        let _ = p_trigger;
        async move { core.list_mod_presets(gid, mid).await.unwrap_or_default() }
    });
    let presets = (presets_res)().unwrap_or_default();

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
                    button {
                        class: if current_tab == Tab::Mesh { "itab active" } else { "itab" },
                        onclick: move |_| tab.set(Tab::Mesh),
                        IconSvg { name: IconName::Deploy, size: 13 }
                        "3D MESH"
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
                                    if !record.config.shader_files.is_empty() {
                                        span { class: "tag", style: "background: rgba(14, 165, 233, 0.15); color: #38bdf8; border: 1px solid rgba(14, 165, 233, 0.4);",
                                            "SHADERS: {record.config.shader_files.len()}"
                                        }
                                    }
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

                        div { class: "info-tags",
                            div { class: "field-label mono", IconSvg { name: IconName::Refresh, size: 13 } "WARDROBE & SHUFFLE CONTROLS" }
                            div { style: "display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 6px; padding: 6px 10px; background: var(--panel-2); border: 1px solid var(--border); border-radius: 4px;",
                                div { style: "display: flex; align-items: center; gap: 8px;",
                                    button {
                                        class: if record.config.shuffle_locked { "toggle-switch on" } else { "toggle-switch" },
                                        title: "Lock from Shuffle-on-Launch",
                                        onclick: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            let current_locked = record.config.shuffle_locked;
                                            let ctx = record.config.context_pool.clone();
                                            move |_| {
                                                let core = store.core.clone();
                                                let gid = gid.clone();
                                                let mid = mid.clone();
                                                let ctx = ctx.clone();
                                                let mut s = store.clone();
                                                spawn(async move {
                                                    match core.update_mod_wardrobe_meta(gid, mid, !current_locked, ctx).await {
                                                        Ok(_) => s.toast(ToastKind::Success, if !current_locked { "Mod locked from shuffle" } else { "Mod unlocked for shuffle" }),
                                                        Err(e) => s.toast(ToastKind::Error, e),
                                                    }
                                                });
                                            }
                                        },
                                        div { class: "toggle-knob" }
                                    }
                                    div {
                                        div { style: "font-size: 11px; font-weight: 600; color: var(--text);", "Lock from Shuffle" }
                                        div { style: "font-size: 9px; color: var(--muted);", "Preserve activation state when rolling skins" }
                                    }
                                }

                                div { style: "display: flex; align-items: center; gap: 6px;",
                                    span { class: "mono", style: "font-size: 10px; color: var(--muted);", "Pool:" }
                                    select {
                                        class: "input select-sm mono",
                                        style: "font-size: 11px; padding: 2px 6px;",
                                        value: "{record.config.context_pool.as_deref().unwrap_or(\"both\")}",
                                        onchange: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            let locked = record.config.shuffle_locked;
                                            move |e: Event<FormData>| {
                                                let pool_val = match e.value().as_str() {
                                                    "land" => Some("land".to_string()),
                                                    "aqua" => Some("aqua".to_string()),
                                                    _ => None,
                                                };
                                                let core = store.core.clone();
                                                let gid = gid.clone();
                                                let mid = mid.clone();
                                                let mut s = store.clone();
                                                spawn(async move {
                                                    match core.update_mod_wardrobe_meta(gid, mid, locked, pool_val).await {
                                                        Ok(_) => s.toast(ToastKind::Success, "Context pool updated"),
                                                        Err(e) => s.toast(ToastKind::Error, e),
                                                    }
                                                });
                                            }
                                        },
                                        option { value: "both", "Both (Universal)" }
                                        option { value: "land", "Land Only" }
                                        option { value: "aqua", "Aqua (Swimming)" }
                                    }
                                }
                            }
                        }

                        if !record.config.keybinds.is_empty() {
                            div { class: "info-tags",
                                div { class: "field-label mono", IconSvg { name: IconName::Settings, size: 13 } "KEYBINDS & CONTROLS (CLICK TO REBIND)" }
                                div { class: "keybinds-list",
                                    for (kname, kb) in record.config.keybinds.iter() {
                                        {
                                            let is_capturing = (capturing_keybind)().as_deref() == Some(kname.as_str());
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            let kname_clone = kname.clone();
                                            let kb_label = kb.label.clone();
                                            let kb_key = kb.key.clone();
                                            let kb_values = kb.values.clone();

                                            rsx! {
                                                div { key: "{kname}", class: "keybind-row mono",
                                                    button {
                                                        class: if is_capturing { "keybind-badge capturing" } else { "keybind-badge" },
                                                        autofocus: is_capturing,
                                                        onclick: {
                                                            let kname_click = kname_clone.clone();
                                                            move |_| {
                                                                if is_capturing {
                                                                    capturing_keybind.set(None);
                                                                } else {
                                                                    capturing_keybind.set(Some(kname_click.clone()));
                                                                }
                                                            }
                                                        },
                                                        onkeydown: move |evt: KeyboardEvent| {
                                                            if is_capturing {
                                                                let key_code = evt.key();
                                                                let raw_key = match key_code {
                                                                    Key::Character(ref c) => c.clone(),
                                                                    Key::ArrowUp => "up".to_string(),
                                                                    Key::ArrowDown => "down".to_string(),
                                                                    Key::ArrowLeft => "left".to_string(),
                                                                    Key::ArrowRight => "right".to_string(),
                                                                    _ => format!("{:?}", key_code).to_lowercase(),
                                                                };
                                                                let mut parts = Vec::new();
                                                                if evt.modifiers().ctrl() { parts.push("ctrl"); }
                                                                if evt.modifiers().alt() { parts.push("alt"); }
                                                                if evt.modifiers().shift() { parts.push("shift"); }
                                                                if !parts.contains(&raw_key.as_str()) {
                                                                    parts.push(&raw_key);
                                                                }
                                                                let new_key_str = parts.join(" ");

                                                                let core = store.core.clone();
                                                                let gid = gid.clone();
                                                                let mid = mid.clone();
                                                                let label = kb_label.clone();
                                                                let mut s = store.clone();
                                                                spawn(async move {
                                                                    match core.update_mod_keybind(gid, mid, label, new_key_str.clone()).await {
                                                                        Ok(_) => s.toast(ToastKind::Success, format!("Keybind updated to [{new_key_str}]")),
                                                                        Err(e) => s.toast(ToastKind::Error, e),
                                                                    }
                                                                    capturing_keybind.set(None);
                                                                });
                                                            }
                                                        },
                                                        if is_capturing { "Press key…" } else { "[{kb_key}]" }
                                                    }
                                                    span { class: "keybind-label", "{kb_label}" }
                                                    if !kb_values.is_empty() {
                                                        span { class: "keybind-values", "({kb_values.join(\", \")})" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        div { class: "info-tags",
                            div { class: "field-label mono", IconSvg { name: IconName::Deploy, size: 13 } "VARIANT PRESETS" }
                            div { class: "preset-pills",
                                for p in presets.iter() {
                                    {
                                        let pid = p.id.clone();
                                        let pname = p.name.clone();
                                        let is_def = p.is_default;
                                        let is_active = if let Some(active_id) = record.config.active_preset_id.as_deref() {
                                            active_id == pid
                                        } else {
                                            is_def
                                        };

                                        let store = store.clone();
                                        let gid = game_id.clone();
                                        let mid = mod_id.clone();

                                        rsx! {
                                            span {
                                                key: "{pid}",
                                                class: if is_active { "preset-pill active" } else { "preset-pill" },
                                                onclick: {
                                                    let store = store.clone();
                                                    let gid = gid.clone();
                                                    let mid = mid.clone();
                                                    let pid_act = pid.clone();
                                                    move |_| {
                                                        let core = store.core.clone();
                                                        let gid = gid.clone();
                                                        let mid = mid.clone();
                                                        let target_id = if is_def { None } else { Some(pid_act.clone()) };
                                                        let mut s = store.clone();
                                                        spawn(async move {
                                                            match core.activate_mod_preset(gid, mid, target_id).await {
                                                                Ok(_) => preset_refresh.set(preset_refresh() + 1),
                                                                Err(e) => s.toast(ToastKind::Error, e),
                                                            }
                                                        });
                                                    }
                                                },
                                                "{pname}"
                                                if !is_def {
                                                    button {
                                                        class: "preset-pill-x",
                                                        title: "Delete preset",
                                                        onclick: {
                                                            let store = store.clone();
                                                            let gid = gid.clone();
                                                            let mid = mid.clone();
                                                            let pid_del = pid.clone();
                                                            move |e: MouseEvent| {
                                                                e.stop_propagation();
                                                                let core = store.core.clone();
                                                                let gid = gid.clone();
                                                                let mid = mid.clone();
                                                                let pid = pid_del.clone();
                                                                let mut s = store.clone();
                                                                spawn(async move {
                                                                    match core.delete_mod_preset(gid, mid, pid).await {
                                                                        Ok(_) => preset_refresh.set(preset_refresh() + 1),
                                                                        Err(err) => s.toast(ToastKind::Error, err),
                                                                    }
                                                                });
                                                            }
                                                        },
                                                        IconSvg { name: IconName::Close, size: 10 }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if (creating_preset)() {
                                    input {
                                        class: "input tag-input",
                                        r#type: "text",
                                        placeholder: "Preset name...",
                                        autofocus: true,
                                        value: "{(new_preset_name)()}",
                                        oninput: move |e| new_preset_name.set(e.value()),
                                        onblur: move |_| creating_preset.set(false),
                                        onkeydown: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            move |evt: KeyboardEvent| {
                                                if evt.key() == Key::Enter {
                                                    let name = (new_preset_name)().trim().to_string();
                                                    if !name.is_empty() {
                                                        let core = store.core.clone();
                                                        let gid = gid.clone();
                                                        let mid = mid.clone();
                                                        let mut s = store.clone();
                                                        spawn(async move {
                                                            match core.save_mod_preset(gid, mid, name, None).await {
                                                                Ok(_) => {
                                                                    s.toast(ToastKind::Success, "Preset captured from d3dx_user.ini");
                                                                    preset_refresh.set(preset_refresh() + 1);
                                                                }
                                                                Err(e) => s.toast(ToastKind::Error, e),
                                                            }
                                                        });
                                                    }
                                                    new_preset_name.set(String::new());
                                                    creating_preset.set(false);
                                                }
                                            }
                                        },
                                    }
                                } else {
                                    button {
                                        class: "btn btn-sm",
                                        title: "Capture current live settings from d3dx_user.ini as a new preset",
                                        onclick: move |_| creating_preset.set(true),
                                        IconSvg { name: IconName::Add, size: 12 }
                                        "CAPTURE CURRENT"
                                    }
                                }
                            }
                        }

                        div { class: "info-tags",
                            div { style: "display: flex; align-items: center; justify-content: space-between;",
                                div { class: "field-label mono", IconSvg { name: IconName::Settings, size: 13 } "INI ANCHORS & BANNER SUPPRESSION" }
                                button {
                                    class: if record.config.suppress_creditinfo { "toggle-switch on" } else { "toggle-switch" },
                                    title: "Suppress $creditinfo on-screen banners for this mod",
                                    onclick: {
                                        let store = store.clone();
                                        let gid = game_id.clone();
                                        let mid = mod_id.clone();
                                        let anchors = record.config.ini_anchors.clone();
                                        let current_val = record.config.suppress_creditinfo;
                                        move |_| {
                                            let core = store.core.clone();
                                            let gid = gid.clone();
                                            let mid = mid.clone();
                                            let anchors = anchors.clone();
                                            let mut s = store.clone();
                                            spawn(async move {
                                                let _ = core.update_mod_anchors(gid, mid, anchors, !current_val).await;
                                                s.toast(ToastKind::Success, if !current_val { "$creditinfo suppressed" } else { "$creditinfo restored" });
                                            });
                                        }
                                    },
                                    div { class: "toggle-knob" }
                                }
                            }
                            div { style: "font-size: 10px; color: var(--text-sec); margin-top: 2px;",
                                "Suppress on-screen $creditinfo banners or pin permanent [Section] key = value overrides"
                            }

                            if !record.config.ini_anchors.is_empty() {
                                div { class: "anchors-list",
                                    for (idx, anchor) in record.config.ini_anchors.iter().enumerate() {
                                        {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            let all_anchors = record.config.ini_anchors.clone();
                                            let suppress = record.config.suppress_creditinfo;
                                            let a_sec = anchor.section.clone();
                                            let a_key = anchor.key.clone();
                                            let a_val = anchor.value.clone();
                                            let a_en = anchor.enabled;

                                            rsx! {
                                                div { key: "{idx}", class: "anchor-row mono",
                                                    div { class: "anchor-info",
                                                        button {
                                                            class: if a_en { "toggle-switch on" } else { "toggle-switch" },
                                                            style: "transform: scale(0.8);",
                                                            onclick: {
                                                                let store = store.clone();
                                                                let gid = gid.clone();
                                                                let mid = mid.clone();
                                                                let mut next_anchors = all_anchors.clone();
                                                                move |_| {
                                                                    next_anchors[idx].enabled = !next_anchors[idx].enabled;
                                                                    let core = store.core.clone();
                                                                    let gid = gid.clone();
                                                                    let mid = mid.clone();
                                                                    let na = next_anchors.clone();
                                                                    spawn(async move {
                                                                        let _ = core.update_mod_anchors(gid, mid, na, suppress).await;
                                                                    });
                                                                }
                                                            },
                                                            div { class: "toggle-knob" }
                                                        }
                                                        span { style: "color: var(--accent);", "[{a_sec}]" }
                                                        span { "{a_key} = {a_val}" }
                                                    }
                                                    button {
                                                        class: "icon-btn icon-danger",
                                                        title: "Remove anchor",
                                                        onclick: {
                                                            let store = store.clone();
                                                            let gid = gid.clone();
                                                            let mid = mid.clone();
                                                            let mut next_anchors = all_anchors.clone();
                                                            move |_| {
                                                                next_anchors.remove(idx);
                                                                let core = store.core.clone();
                                                                let gid = gid.clone();
                                                                let mid = mid.clone();
                                                                let na = next_anchors.clone();
                                                                spawn(async move {
                                                                    let _ = core.update_mod_anchors(gid, mid, na, suppress).await;
                                                                });
                                                            }
                                                        },
                                                        IconSvg { name: IconName::Trash, size: 13 }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if (adding_anchor)() {
                                div { style: "display: flex; gap: 6px; margin-top: 6px; align-items: center; flex-wrap: wrap;",
                                    input {
                                        class: "input",
                                        style: "flex: 1; min-width: 90px; font-size: 11px;",
                                        placeholder: "Section (e.g. TextureOverrideBody)",
                                        value: "{(new_anchor_sec)()}",
                                        oninput: move |e| new_anchor_sec.set(e.value()),
                                    }
                                    input {
                                        class: "input",
                                        style: "flex: 1; min-width: 70px; font-size: 11px;",
                                        placeholder: "Key (e.g. filter_index)",
                                        value: "{(new_anchor_key)()}",
                                        oninput: move |e| new_anchor_key.set(e.value()),
                                    }
                                    input {
                                        class: "input",
                                        style: "flex: 1; min-width: 60px; font-size: 11px;",
                                        placeholder: "Value (e.g. 1)",
                                        value: "{(new_anchor_val)()}",
                                        oninput: move |e| new_anchor_val.set(e.value()),
                                    }
                                    button {
                                        class: "btn btn-sm btn-accent",
                                        onclick: {
                                            let store = store.clone();
                                            let gid = game_id.clone();
                                            let mid = mod_id.clone();
                                            let mut all_anchors = record.config.ini_anchors.clone();
                                            let suppress = record.config.suppress_creditinfo;
                                            move |_| {
                                                let sec = (new_anchor_sec)().trim().to_string();
                                                let k = (new_anchor_key)().trim().to_string();
                                                let v = (new_anchor_val)().trim().to_string();
                                                if !sec.is_empty() && !k.is_empty() {
                                                    all_anchors.push(IniAnchor {
                                                        section: sec,
                                                        key: k,
                                                        value: v,
                                                        enabled: true,
                                                    });
                                                    let core = store.core.clone();
                                                    let gid = gid.clone();
                                                    let mid = mid.clone();
                                                    let na = all_anchors.clone();
                                                    let mut s = store.clone();
                                                    spawn(async move {
                                                        let _ = core.update_mod_anchors(gid, mid, na, suppress).await;
                                                        s.toast(ToastKind::Success, "Anchor added");
                                                    });
                                                    new_anchor_sec.set(String::new());
                                                    new_anchor_key.set(String::new());
                                                    new_anchor_val.set(String::new());
                                                    adding_anchor.set(false);
                                                }
                                            }
                                        },
                                        "SAVE"
                                    }
                                    button {
                                        class: "btn btn-sm",
                                        onclick: move |_| adding_anchor.set(false),
                                        "CANCEL"
                                    }
                                }
                            } else {
                                button {
                                    class: "btn btn-sm",
                                    style: "margin-top: 6px;",
                                    onclick: move |_| adding_anchor.set(true),
                                    IconSvg { name: IconName::Add, size: 12 }
                                    "ADD INI ANCHOR"
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
                                disabled: (exporting_clean)(),
                                onclick: {
                                    let store = store.clone();
                                    let mid = mod_id.clone();
                                    let mname = record.meta.name.clone();
                                    move |_| {
                                        let core = store.core.clone();
                                        let mid = mid.clone();
                                        let mname = mname.clone();
                                        let mut s = store.clone();
                                        spawn(async move {
                                            let default_name = format!("{}_clean.zip", mname.replace(' ', "_"));
                                            if let Some(dest) = pick_save_file("Export Clean Mod Archive", &default_name).await {
                                                exporting_clean.set(true);
                                                let dest_str = dest.display().to_string();
                                                match core.export_clean_mod(mid, dest_str.clone()).await {
                                                    Ok(count) => {
                                                        s.toast(ToastKind::Success, format!("Clean mod exported ({count} files) to {dest_str}"));
                                                    }
                                                    Err(e) => s.toast(ToastKind::Error, e),
                                                }
                                                exporting_clean.set(false);
                                            }
                                        });
                                    }
                                },
                                IconSvg { name: IconName::Download, size: 15 }
                                if (exporting_clean)() { "EXPORTING…" } else { "EXPORT CLEAN .ZIP" }
                            }
                            button {
                                class: "btn btn-block",
                                disabled: (syncing_dvt)(),
                                title: "Transfer updated draw counts and .buf/.ib files from Blender export without clobbering toggle logic",
                                onclick: {
                                    let store = store.clone();
                                    let mid = mod_id.clone();
                                    move |_| {
                                        let core = store.core.clone();
                                        let mid = mid.clone();
                                        let mut s = store.clone();
                                        spawn(async move {
                                            if let Some(export_dir) = pick_folder("Select Blender Export Folder").await {
                                                syncing_dvt.set(true);
                                                let exp_str = export_dir.display().to_string();
                                                match core.sync_mod_draw_values(mid, exp_str).await {
                                                    Ok(rep) => {
                                                        s.toast(
                                                            ToastKind::Success,
                                                            format!(
                                                                "DVT Sync complete: {} draw(s) updated, {} buffer(s) copied (Backup created)",
                                                                rep.updated_draws, rep.copied_buffers
                                                            ),
                                                        );
                                                        backup_refresh.set(backup_refresh() + 1);
                                                    }
                                                    Err(e) => s.toast(ToastKind::Error, e),
                                                }
                                                syncing_dvt.set(false);
                                            }
                                        });
                                    }
                                },
                                IconSvg { name: IconName::Deploy, size: 15 }
                                if (syncing_dvt)() { "SYNCING DVT…" } else { "SYNC BLENDER EXPORT (DVT)" }
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

                if current_tab == Tab::Mesh {
                    MeshPreviewView { mod_id: mod_id.clone() }
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

#[component]
fn MeshPreviewView(mod_id: String) -> Element {
    let store = use_context::<Store>();
    let mut mesh_data = use_signal(|| None::<MeshPreviewData>);
    let mut loading = use_signal(|| false);
    let mut is_wireframe = use_signal(|| false);
    let mut error_msg = use_signal(|| None::<String>);

    let data = (mesh_data)();
    let is_loading = (loading)();
    let wire = (is_wireframe)();
    let err = (error_msg)();

    use_effect(move || {
        if let Some(d) = (mesh_data)() {
            let eval_script = generate_webgl_init_script(&d.positions, &d.indices);
            let _ = document::eval(&eval_script);
        }
    });

    rsx! {
        div { class: "inspector-files", style: "padding: 12px; display: flex; flex-direction: column; gap: 10px;",
            if data.is_none() {
                div {
                    style: "padding: 32px 16px; background: var(--panel-2); border: 1px dashed var(--border); border-radius: 6px; text-align: center; display: flex; flex-direction: column; align-items: center; gap: 12px;",
                    IconSvg { name: IconName::Deploy, size: 40 }
                    div {
                        h4 { style: "font-size: 13px; font-weight: 600; color: var(--text); margin-bottom: 4px;", "3D MESH GEOMETRY PREVIEW" }
                        p { style: "font-size: 11px; color: var(--muted); max-width: 380px; margin: 0 auto;",
                            "Extract and render 3DMigoto vertex (.buf) and index (.ib) geometry in real-time WebGL directly inside YAGO."
                        }
                    }

                    if let Some(e) = err {
                        div { class: "mono", style: "font-size: 11px; color: var(--danger); padding: 6px 12px; background: rgba(239, 68, 68, 0.1); border-radius: 4px;",
                            "{e}"
                        }
                    }

                    button {
                        class: "btn btn-accent",
                        disabled: is_loading,
                        onclick: {
                            let store = store.clone();
                            let mid = mod_id.clone();
                            move |_| {
                                let core = store.core.clone();
                                let mid = mid.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    loading.set(true);
                                    error_msg.set(None);
                                    match core.get_mod_mesh_preview(mid).await {
                                        Ok(parsed) => {
                                            s.toast(ToastKind::Success, format!("Parsed {} triangles", parsed.triangle_count));
                                            mesh_data.set(Some(parsed));
                                        }
                                        Err(e) => {
                                            s.toast(ToastKind::Error, e.clone());
                                            error_msg.set(Some(e));
                                        }
                                    }
                                    loading.set(false);
                                });
                            }
                        },
                        IconSvg { name: IconName::Deploy, size: 14 }
                        if is_loading { "PARSING GEOMETRY…" } else { "LOAD 3D PREVIEW" }
                    }
                }
            } else if let Some(d) = data {
                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    div { style: "display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px;",
                        div { style: "display: flex; align-items: center; gap: 8px;",
                            span { class: "tag mono", "{d.vertex_count} VERTICES" }
                            span { class: "tag mono", "{d.triangle_count} TRIS" }
                            for src in d.source_files.iter() {
                                span { class: "tag tag-outline mono", style: "font-size: 10px;", "{src}" }
                            }
                        }
                        div { style: "display: flex; align-items: center; gap: 6px;",
                            button {
                                class: "btn btn-sm",
                                onclick: move |_| {
                                    let next_wire = !wire;
                                    is_wireframe.set(next_wire);
                                    let _ = document::eval(&format!("if (window.__yago_set_wireframe) window.__yago_set_wireframe({});", next_wire));
                                },
                                if wire { "SHADED" } else { "WIREFRAME" }
                            }
                            button {
                                class: "btn btn-sm",
                                onclick: move |_| {
                                    let _ = document::eval("if (window.__yago_reset_cam) window.__yago_reset_cam();");
                                },
                                "RESET CAM"
                            }
                            button {
                                class: "btn btn-sm",
                                title: "Reload mesh geometry",
                                onclick: {
                                    let store = store.clone();
                                    let mid = mod_id.clone();
                                    move |_| {
                                        let core = store.core.clone();
                                        let mid = mid.clone();
                                        spawn(async move {
                                            if let Ok(parsed) = core.get_mod_mesh_preview(mid).await {
                                                mesh_data.set(Some(parsed));
                                            }
                                        });
                                    }
                                },
                                IconSvg { name: IconName::Refresh, size: 12 }
                            }
                        }
                    }

                    canvas {
                        id: "yago-mesh-canvas",
                        style: "width: 100%; height: 380px; background: #070a13; border: 1px solid var(--border); border-radius: 4px; display: block; touch-action: none; cursor: grab;",
                    }

                    div { class: "mono", style: "font-size: 10px; color: var(--muted); text-align: center;",
                        "Left Click + Drag to Orbit • Scroll Wheel to Zoom"
                    }
                }
            }
        }
    }
}

fn generate_webgl_init_script(positions: &[f32], indices: &[u32]) -> String {
    format!(
        r#"(function() {{
            const canvas = document.getElementById('yago-mesh-canvas');
            if (!canvas) return;
            const gl = canvas.getContext('webgl2') || canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
            if (!gl) {{ console.error('WebGL not supported'); return; }}
            gl.getExtension('OES_element_index_uint');

            const positions = new Float32Array({positions:?});
            const indices = new Uint32Array({indices:?});

            const lineIndices = new Uint32Array(indices.length * 2);
            let lIdx = 0;
            for (let i = 0; i < indices.length; i += 3) {{
                lineIndices[lIdx++] = indices[i];
                lineIndices[lIdx++] = indices[i+1];
                lineIndices[lIdx++] = indices[i+1];
                lineIndices[lIdx++] = indices[i+2];
                lineIndices[lIdx++] = indices[i+2];
                lineIndices[lIdx++] = indices[i];
            }}

            const vsSource = `
                attribute vec3 aPos;
                uniform mat4 uMatrix;
                varying vec3 vNormal;
                void main() {{
                    vNormal = normalize(aPos);
                    gl_Position = uMatrix * vec4(aPos, 1.0);
                }}
            `;

            const fsSource = `
                precision mediump float;
                varying vec3 vNormal;
                uniform bool uWireframe;
                void main() {{
                    if (uWireframe) {{
                        gl_FragColor = vec4(0.2, 0.85, 1.0, 1.0);
                    }} else {{
                        vec3 light = normalize(vec3(0.5, 0.8, 1.0));
                        float diff = max(dot(vNormal, light), 0.1);
                        vec3 col = vec3(0.65, 0.75, 0.88) * (diff * 0.8 + 0.25);
                        gl_FragColor = vec4(col, 1.0);
                    }}
                }}
            `;

            function createShader(gl, type, source) {{
                const s = gl.createShader(type);
                gl.shaderSource(s, source);
                gl.compileShader(s);
                return s;
            }}

            const vs = createShader(gl, gl.VERTEX_SHADER, vsSource);
            const fs = createShader(gl, gl.FRAGMENT_SHADER, fsSource);
            const prog = gl.createProgram();
            gl.attachShader(prog, vs);
            gl.attachShader(prog, fs);
            gl.linkProgram(prog);
            gl.useProgram(prog);

            const aPos = gl.getAttribLocation(prog, 'aPos');
            const uMatrix = gl.getUniformLocation(prog, 'uMatrix');
            const uWireframe = gl.getUniformLocation(prog, 'uWireframe');

            const posBuf = gl.createBuffer();
            gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
            gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);
            gl.enableVertexAttribArray(aPos);
            gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);

            const idxBuf = gl.createBuffer();
            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, idxBuf);
            gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);

            const lineBuf = gl.createBuffer();
            gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, lineBuf);
            gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, lineIndices, gl.STATIC_DRAW);

            let rotX = 0.2;
            let rotY = 0.5;
            let dist = 2.4;
            let isWire = false;
            let dragging = false;
            let lastX = 0, lastY = 0;

            function multiply(a, b) {{
                let out = new Float32Array(16);
                for (let i = 0; i < 4; i++) {{
                    for (let j = 0; j < 4; j++) {{
                        out[j * 4 + i] =
                            a[0 * 4 + i] * b[j * 4 + 0] +
                            a[1 * 4 + i] * b[j * 4 + 1] +
                            a[2 * 4 + i] * b[j * 4 + 2] +
                            a[3 * 4 + i] * b[j * 4 + 3];
                    }}
                }}
                return out;
            }}

            function render() {{
                const dpr = window.devicePixelRatio || 1;
                const w = canvas.clientWidth * dpr;
                const h = canvas.clientHeight * dpr;
                if (canvas.width !== w || canvas.height !== h) {{
                    canvas.width = w;
                    canvas.height = h;
                }}
                gl.viewport(0, 0, canvas.width, canvas.height);
                gl.clearColor(0.04, 0.06, 0.11, 1.0);
                gl.enable(gl.DEPTH_TEST);
                gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

                const aspect = canvas.width / (canvas.height || 1);
                const fovy = 45 * Math.PI / 180;
                const near = 0.1, far = 100.0;
                const f = 1.0 / Math.tan(fovy / 2);
                const nf = 1 / (near - far);
                const proj = new Float32Array([
                    f / aspect, 0, 0, 0,
                    0, f, 0, 0,
                    0, 0, (far + near) * nf, -1,
                    0, 0, (2 * far * near) * nf, 0
                ]);

                const view = new Float32Array([
                    1, 0, 0, 0,
                    0, 1, 0, 0,
                    0, 0, 1, 0,
                    0, 0, -dist, 1
                ]);

                const cosX = Math.cos(rotX), sinX = Math.sin(rotX);
                const rx = new Float32Array([
                    1, 0, 0, 0,
                    0, cosX, sinX, 0,
                    0, -sinX, cosX, 0,
                    0, 0, 0, 1
                ]);

                const cosY = Math.cos(rotY), sinY = Math.sin(rotY);
                const ry = new Float32Array([
                    cosY, 0, -sinY, 0,
                    0, 1, 0, 0,
                    sinY, 0, cosY, 0,
                    0, 0, 0, 1
                ]);

                const model = multiply(ry, rx);
                const mv = multiply(view, model);
                const mvp = multiply(proj, mv);

                gl.useProgram(prog);
                gl.uniformMatrix4fv(uMatrix, false, mvp);
                gl.uniform1i(uWireframe, isWire ? 1 : 0);

                if (isWire) {{
                    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, lineBuf);
                    gl.drawElements(gl.LINES, lineIndices.length, gl.UNSIGNED_INT, 0);
                }} else {{
                    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, idxBuf);
                    gl.drawElements(gl.TRIANGLES, indices.length, gl.UNSIGNED_INT, 0);
                }}
            }}

            canvas.onpointerdown = (e) => {{
                dragging = true;
                lastX = e.clientX;
                lastY = e.clientY;
                canvas.style.cursor = 'grabbing';
            }};

            window.onpointerup = () => {{
                dragging = false;
                if (canvas) canvas.style.cursor = 'grab';
            }};

            window.onpointermove = (e) => {{
                if (!dragging) return;
                const dx = e.clientX - lastX;
                const dy = e.clientY - lastY;
                lastX = e.clientX;
                lastY = e.clientY;
                rotY += dx * 0.01;
                rotX = Math.max(-1.5, Math.min(1.5, rotX + dy * 0.01));
                render();
            }};

            canvas.onwheel = (e) => {{
                dist = Math.max(0.6, Math.min(8.0, dist + e.deltaY * 0.003));
                render();
                e.preventDefault();
            }};

            window.__yago_set_wireframe = (val) => {{
                isWire = val;
                render();
            }};

            window.__yago_reset_cam = () => {{
                rotX = 0.2;
                rotY = 0.5;
                dist = 2.4;
                render();
            }};

            render();
        }})();"#
    )
}
