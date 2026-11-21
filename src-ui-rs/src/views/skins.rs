use std::collections::HashMap;

use dioxus::prelude::*;
use storage::queries::{CharacterGroup, ModSnippet};
use uuid::Uuid;

use crate::icons::{IconName, IconSvg};
use crate::modutil::{active_profile, is_nsfw};
use crate::picker::pick_file;
use crate::state::{Store, ToastKind};

#[component]
pub fn SkinsView() -> Element {
    let store = use_context::<Store>();
    let mut search = use_signal(String::new);
    let mut selected_char = use_signal(|| None::<String>);
    let mut importing = use_signal(|| false);

    let Some(game) = store.selected_game() else {
        return rsx! {
            div { class: "overview-empty",
                IconSvg { name: IconName::Skins, size: 32 }
                h2 { "No title selected" }
                p { "Select a game to manage skins." }
            }
        };
    };

    let game_id = game.id.clone();
    let settings = (store.settings)();
    let stream_safe = settings.stream_safe;
    let is_importing = (importing)();

    let core = store.core.clone();
    let gid_res = game_id.clone();
    let roster_res = use_resource(move || {
        let core = core.clone();
        let gid = gid_res.clone();
        async move {
            core.get_skin_inventory(gid).await.unwrap_or_default()
        }
    });

    let roster: HashMap<String, CharacterGroup> = (roster_res)().unwrap_or_default();
    let term = (search)().to_lowercase();
    let sel = (selected_char)();

    let mut filtered: Vec<(String, CharacterGroup)> = roster
        .into_iter()
        .filter(|(name, _)| term.is_empty() || name.to_lowercase().contains(&term))
        .collect();
    filtered.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let selected_group = sel.as_ref().and_then(|name| {
        filtered.iter().find(|(n, _)| n == name).map(|(n, g)| (n.clone(), g.clone()))
    });

    rsx! {
        div { class: "skins-view",
            div { class: "skins-header",
                div { class: "skins-title",
                    IconSvg { name: IconName::Skins, size: 24 }
                    h1 { "SKIN GALLERY" }
                }
                div { class: "skins-header-actions",
                    div { class: "mods-search",
                        IconSvg { name: IconName::Search, size: 15 }
                        input {
                            class: "input",
                            r#type: "text",
                            placeholder: "Search characters...",
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
                                    let Some(path) = pick_file("Select Mod Archive").await else { return };
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
                        if is_importing { "IMPORTING…" } else { "IMPORT ASSET" }
                    }
                }
            }

            div { class: "skins-body",
                if filtered.is_empty() {
                    div { class: "skins-empty",
                        IconSvg { name: IconName::Skins, size: 48 }
                        p { "No characters found" }
                        span { "Import character skins to populate the gallery." }
                    }
                }
                div { class: "skins-grid",
                    for (name, group) in filtered.iter() {
                        CharacterCard {
                            key: "{name}",
                            name: name.clone(),
                            active_count: group.active_cycle.len(),
                            total_count: group.skins.len(),
                            is_nsfw: group.skins.iter().any(|s| is_nsfw(&s.tags)),
                            stream_safe,
                            onclick: {
                                let name = name.clone();
                                move |_| selected_char.set(Some(name.clone()))
                            },
                        }
                    }
                }
            }

            if let Some((char_name, group)) = selected_group {
                CycleEditor {
                    character_name: char_name,
                    group,
                    game_id: game_id.clone(),
                    stream_safe,
                    onclose: move |_| selected_char.set(None),
                }
            }
        }
    }
}

// ─── CharacterCard ───────────────────────────────────────────────

#[component]
fn CharacterCard(
    name: String,
    active_count: usize,
    total_count: usize,
    is_nsfw: bool,
    stream_safe: bool,
    onclick: EventHandler<()>,
) -> Element {
    let blur = is_nsfw && stream_safe;

    rsx! {
        button {
            class: "char-card",
            onclick: move |_| onclick.call(()),
            div { class: "char-card-art",
                div { class: "char-card-placeholder",
                    IconSvg { name: IconName::Skins, size: 40 }
                }
                div { class: "char-card-shade" }
            }
            div { class: "char-card-badges",
                if active_count > 1 {
                    span { class: "char-badge accent",
                        IconSvg { name: IconName::Deploy, size: 10 }
                        "ACTIVE"
                    }
                }
                if is_nsfw {
                    span { class: "char-badge danger", "SOVEREIGN" }
                }
            }
            div { class: "char-card-foot",
                h3 { class: if blur { "char-card-name blurred" } else { "char-card-name" }, "{name}" }
                div { class: "char-card-stats mono",
                    IconSvg { name: IconName::Mods, size: 12 }
                    span { "{active_count} / {total_count} Variants" }
                }
            }
        }
    }
}

// ─── CycleEditor (slide-out panel) ──────────────────────────────

#[component]
fn CycleEditor(
    character_name: String,
    group: CharacterGroup,
    game_id: String,
    stream_safe: bool,
    onclose: EventHandler<()>,
) -> Element {
    let store = use_context::<Store>();
    let mut search_term = use_signal(String::new);

    let db = store.db(&game_id);
    let game = store.game(&game_id);
    let profile = db.as_ref().and_then(|d| game.as_ref().and_then(|g| active_profile(d, g).cloned()));
    let load_order = profile.as_ref().map(|p| p.load_order.clone()).unwrap_or_default();

    let mut sorted_skins: Vec<ModSnippet> = group.skins.clone();
    sorted_skins.sort_by_key(|s| {
        let uid = Uuid::parse_str(&s.id).unwrap_or_default();
        load_order.iter().position(|id| *id == uid).unwrap_or(9999)
    });

    let term = (search_term)().to_lowercase();
    let filtered: Vec<&ModSnippet> = sorted_skins
        .iter()
        .filter(|s| term.is_empty() || s.name.to_lowercase().contains(&term))
        .collect();

    rsx! {
        div {
            class: "cycle-overlay",
            onclick: move |_| onclose.call(()),
            div {
                class: "cycle-panel",
                onclick: move |evt| evt.stop_propagation(),

                div { class: "cycle-head",
                    div { class: "cycle-head-left",
                        div { class: "cycle-icon",
                            IconSvg { name: IconName::Mods, size: 24 }
                        }
                        div {
                            h2 { class: "cycle-title", "{character_name}" }
                            p { class: "cycle-sub mono", "CYCLE SEQUENCE & VARIANTS" }
                        }
                    }
                    button { class: "modal-x", onclick: move |_| onclose.call(()),
                        IconSvg { name: IconName::Close, size: 20 }
                    }
                }

                div { class: "cycle-toolbar",
                    div { class: "cycle-info mono",
                        IconSvg { name: IconName::Check, size: 10 }
                        "Slot 0: Vanilla"
                    }
                    div { class: "cycle-bulk",
                        button {
                            class: "btn-link",
                            onclick: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                let skins = group.skins.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let skins = skins.clone();
                                    let mut s = store.clone();
                                    spawn(async move {
                                        for skin in &skins {
                                            if !skin.enabled {
                                                let _ = core.toggle_mod(gid.clone(), skin.id.clone(), true).await;
                                            }
                                        }
                                        s.toast(ToastKind::Success, "All variants enabled");
                                    });
                                }
                            },
                            "ENABLE ALL"
                        }
                        button {
                            class: "btn-link danger",
                            onclick: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                let skins = group.skins.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let skins = skins.clone();
                                    let mut s = store.clone();
                                    spawn(async move {
                                        for skin in &skins {
                                            if skin.enabled {
                                                let _ = core.toggle_mod(gid.clone(), skin.id.clone(), false).await;
                                            }
                                        }
                                        s.toast(ToastKind::Success, "All variants disabled");
                                    });
                                }
                            },
                            "DISABLE ALL"
                        }
                    }
                }

                div { class: "cycle-search",
                    IconSvg { name: IconName::Search, size: 14 }
                    input {
                        class: "input",
                        r#type: "text",
                        placeholder: "Filter variants...",
                        value: "{(search_term)()}",
                        oninput: move |evt| search_term.set(evt.value()),
                    }
                }

                div { class: "cycle-list",
                    for (idx, skin) in filtered.iter().enumerate() {
                        CycleSkinRow {
                            key: "{skin.id}",
                            skin: (*skin).clone(),
                            game_id: game_id.clone(),
                            cycle_pos: group.active_cycle.iter().position(|id| id == &skin.id),
                            stream_safe,
                            can_move_up: idx > 0,
                            can_move_down: idx < filtered.len() - 1,
                        }
                    }
                    if filtered.is_empty() {
                        div { class: "cycle-empty",
                            IconSvg { name: IconName::Mods, size: 40 }
                            p { "No variants detected" }
                        }
                    }
                }

                div { class: "cycle-foot",
                    button {
                        class: "btn btn-accent btn-block",
                        onclick: move |_| onclose.call(()),
                        "RETURN TO GALLERY"
                    }
                }
            }
        }
    }
}

#[component]
fn CycleSkinRow(
    skin: ModSnippet,
    game_id: String,
    cycle_pos: Option<usize>,
    stream_safe: bool,
    can_move_up: bool,
    can_move_down: bool,
) -> Element {
    let store = use_context::<Store>();
    let nsfw = is_nsfw(&skin.tags);
    let blur = nsfw && stream_safe;

    rsx! {
        div {
            class: if skin.enabled { "cycle-row enabled" } else { "cycle-row" },

            div { class: "cycle-reorder",
                button {
                    class: "icon-btn",
                    disabled: !can_move_up,
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        let sid = skin.id.clone();
                        move |_| {
                            reorder_skin(store.clone(), gid.clone(), sid.clone(), -1);
                        }
                    },
                    IconSvg { name: IconName::ChevronDown, size: 14 }
                }
                button {
                    class: "icon-btn",
                    disabled: !can_move_down,
                    onclick: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        let sid = skin.id.clone();
                        move |_| {
                            reorder_skin(store.clone(), gid.clone(), sid.clone(), 1);
                        }
                    },
                    IconSvg { name: IconName::ChevronRight, size: 14 }
                }
            }

            button {
                class: "cycle-toggle",
                onclick: {
                    let store = store.clone();
                    let gid = game_id.clone();
                    let sid = skin.id.clone();
                    let en = skin.enabled;
                    move |_| {
                        let core = store.core.clone();
                        let gid = gid.clone();
                        let sid = sid.clone();
                        let s = store.clone();
                        s.call(async move { core.toggle_mod(gid, sid, !en).await }, |_, _| {});
                    }
                },
                if skin.enabled {
                    div { class: "cycle-check",
                        IconSvg { name: IconName::Check, size: 18 }
                        if let Some(pos) = cycle_pos {
                            span { class: "cycle-pos", "{pos + 1}" }
                        }
                    }
                } else {
                    div { class: "cycle-unchecked" }
                }
            }

            div { class: "cycle-info",
                span { class: if blur { "cycle-name blurred" } else { "cycle-name" }, "{skin.name}" }
                if nsfw {
                    span { class: "char-badge danger", "SOVEREIGN" }
                }
            }

            button {
                class: "icon-btn icon-danger cycle-delete",
                title: "Move to Trash",
                onclick: {
                    let store = store.clone();
                    let sid = skin.id.clone();
                    move |_| {
                        let core = store.core.clone();
                        let sid = sid.clone();
                        let s = store.clone();
                        s.call(async move { core.delete_mod(sid).await }, |s, _| {
                            s.toast(ToastKind::Success, "Mod moved to Recycle Bin / Trash")
                        });
                    }
                },
                IconSvg { name: IconName::Trash, size: 16 }
            }
        }
    }
}

fn reorder_skin(store: Store, game_id: String, mod_id: String, delta: i32) {
    let Some(db) = store.db(&game_id) else { return };
    let Some(game) = store.game(&game_id) else { return };
    let Some(profile) = active_profile(&db, &game) else { return };
    let mut order = profile.load_order.clone();
    let Some(uid) = Uuid::parse_str(&mod_id).ok() else { return };
    let Some(idx) = order.iter().position(|id| *id == uid) else { return };
    let new_idx = (idx as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
    if new_idx == idx { return; }
    order.swap(idx, new_idx);

    let core = store.core.clone();
    let s = store.clone();
    s.call(async move { core.set_load_order(game_id, order).await }, |_, _| {});
}
