use crate::icons::{IconName, IconSvg};
use crate::state::{Store, ToastKind, View};
use dioxus::prelude::*;

#[derive(Clone)]
pub struct PaletteAction {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub category: &'static str,
    pub icon: IconName,
}

#[component]
pub fn CommandPalette(is_open: Signal<bool>) -> Element {
    let store = use_context::<Store>();
    let mut query = use_signal(String::new);
    let mut selected_idx = use_signal(|| 0usize);

    if !(is_open)() {
        return rsx! {};
    }

    let search_term = (query)().trim().to_lowercase();
    let selected_game = store.selected_game();

    // 1. Build all available actions
    let mut actions: Vec<PaletteAction> = Vec::new();

    // Active game actions
    if let Some(game) = &selected_game {
        let gname = &game.name;
        actions.push(PaletteAction {
            id: "act:launch".to_string(),
            title: format!("Launch {}", gname),
            subtitle: Some("Start game with active profile & mods".to_string()),
            category: "Active Game",
            icon: IconName::Launch,
        });
        actions.push(PaletteAction {
            id: "act:hotreload".to_string(),
            title: "Deploy & Hot Reload (F10)".to_string(),
            subtitle: Some("Redeploy virtual mods and send F10 to game window".to_string()),
            category: "Active Game",
            icon: IconName::Deploy,
        });
        actions.push(PaletteAction {
            id: "act:shuffle".to_string(),
            title: "Shuffle Character Skins".to_string(),
            subtitle: Some("Roll 1 random unlocked skin per character".to_string()),
            category: "Active Game",
            icon: IconName::Refresh,
        });
        actions.push(PaletteAction {
            id: "act:wish".to_string(),
            title: "Copy Wish / Signal / Convene URL".to_string(),
            subtitle: Some("Extract current gacha authkey URL from webCaches".to_string()),
            category: "Active Game",
            icon: IconName::Copy,
        });
        actions.push(PaletteAction {
            id: "act:audit".to_string(),
            title: "Run Graphics Compatibility Audit".to_string(),
            subtitle: Some("Scan Engine.ini and d3dx.ini for rogue settings".to_string()),
            category: "Active Game",
            icon: IconName::Shield,
        });
        actions.push(PaletteAction {
            id: "act:updates".to_string(),
            title: "Check GameBanana Updates".to_string(),
            subtitle: Some("Query GameBanana API for newer mod versions".to_string()),
            category: "Active Game",
            icon: IconName::Download,
        });
    }

    // Navigation
    actions.push(PaletteAction {
        id: "nav:overview".to_string(),
        title: "Go to Overview".to_string(),
        subtitle: Some("View game status, hero banner & quick launch".to_string()),
        category: "Navigation",
        icon: IconName::Home,
    });
    actions.push(PaletteAction {
        id: "nav:mods".to_string(),
        title: "Go to Mods".to_string(),
        subtitle: Some("Browse, order, and toggle installed mods".to_string()),
        category: "Navigation",
        icon: IconName::Mods,
    });
    actions.push(PaletteAction {
        id: "nav:skins".to_string(),
        title: "Go to Skin Gallery".to_string(),
        subtitle: Some("Character-centric wardrobe and skin management".to_string()),
        category: "Navigation",
        icon: IconName::Skins,
    });
    actions.push(PaletteAction {
        id: "nav:loadouts".to_string(),
        title: "Go to Loadouts & Profiles".to_string(),
        subtitle: Some("Manage profile configurations and mod matrices".to_string()),
        category: "Navigation",
        icon: IconName::Profiles,
    });
    actions.push(PaletteAction {
        id: "nav:settings".to_string(),
        title: "Go to Global Settings".to_string(),
        subtitle: Some("Theme, paths, wine runner, and safety options".to_string()),
        category: "Navigation",
        icon: IconName::Settings,
    });

    // Installed Games Switcher
    for gid in store.game_ids() {
        if let Some(g) = store.game(&gid) {
            let is_curr = selected_game.as_ref().map(|s| s.id == gid).unwrap_or(false);
            actions.push(PaletteAction {
                id: format!("game:{}", gid),
                title: format!("Switch game to {}", g.name),
                subtitle: Some(if is_curr {
                    "Currently Active".to_string()
                } else {
                    format!("ID: {}", gid)
                }),
                category: "Switch Game",
                icon: IconName::Grid,
            });
        }
    }

    // Safety Emergency Panic
    actions.push(PaletteAction {
        id: "sys:panic".to_string(),
        title: "Emergency Panic Purge (Alt+F12)".to_string(),
        subtitle: Some(
            "Instantly purge all deployed mods and reload vanilla game state".to_string(),
        ),
        category: "System Safety",
        icon: IconName::Shield,
    });

    // 2. Filter actions by search query
    let filtered: Vec<PaletteAction> = actions
        .into_iter()
        .filter(|a| {
            if search_term.is_empty() {
                return true;
            }
            a.title.to_lowercase().contains(&search_term)
                || a.category.to_lowercase().contains(&search_term)
                || a.subtitle
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&search_term)
        })
        .collect();

    let count = filtered.len();
    let current_idx = if count == 0 {
        0
    } else {
        (selected_idx)().min(count - 1)
    };

    let filtered_clone_key = filtered.clone();

    rsx! {
        div {
            class: "modal-backdrop",
            style: "z-index: 9999; display: flex; align-items: flex-start; justify-content: center; padding-top: 12vh; background: rgba(5, 8, 16, 0.75); backdrop-filter: blur(4px);",
            onclick: move |_| {
                let mut is_open = is_open;
                is_open.set(false);
            },
            div {
                class: "modal-card",
                style: "width: 580px; max-width: 90vw; background: var(--panel); border: 1px solid var(--accent); border-radius: 6px; box-shadow: 0 16px 48px rgba(0,0,0,0.6); overflow: hidden; display: flex; flex-direction: column;",
                onclick: move |e| e.stop_propagation(),
                onkeydown: {
                    let filtered = filtered_clone_key.clone();
                    let store = store.clone();
                    move |evt: KeyboardEvent| {
                        match evt.key() {
                            Key::Escape => {
                                let mut is_open = is_open;
                                is_open.set(false);
                            }
                            Key::ArrowDown => {
                                if count > 0 {
                                    selected_idx.set((current_idx + 1) % count);
                                }
                            }
                            Key::ArrowUp => {
                                if count > 0 {
                                    selected_idx.set((current_idx + count - 1) % count);
                                }
                            }
                            Key::Enter => {
                                if let Some(item) = filtered.get(current_idx) {
                                    run_palette_action(item.clone(), is_open, store.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                },

                // Search Bar Input
                div {
                    style: "display: flex; align-items: center; gap: 12px; padding: 14px 16px; border-bottom: 1px solid var(--border); background: var(--panel-2);",
                    IconSvg { name: IconName::Search, size: 18 }
                    input {
                        class: "input",
                        style: "flex: 1; font-size: 14px; background: transparent; border: none; outline: none; padding: 0;",
                        placeholder: "Type a command, action, or game title... (Esc to close)",
                        autofocus: true,
                        value: "{(query)()}",
                        oninput: move |e| {
                            query.set(e.value());
                            selected_idx.set(0);
                        },
                    }
                    span { class: "mono", style: "font-size: 10px; color: var(--muted); border: 1px solid var(--border); border-radius: 3px; padding: 2px 6px;", "ESC" }
                }

                // Results List
                div {
                    style: "max-height: 380px; overflow-y: auto; display: flex; flex-direction: column; padding: 6px 0;",
                    if filtered.is_empty() {
                        div { style: "padding: 24px; text-align: center; color: var(--muted); font-size: 12px;",
                            "No matching commands found."
                        }
                    } else {
                        for (idx, action) in filtered.iter().enumerate() {
                            {
                                let is_active = idx == current_idx;
                                let action_clone = action.clone();
                                let store_click = store.clone();
                                rsx! {
                                    div {
                                        key: "{action.id}",
                                        style: if is_active {
                                            "padding: 10px 16px; display: flex; align-items: center; justify-content: space-between; background: rgba(34, 211, 238, 0.12); border-left: 3px solid var(--accent); cursor: pointer;"
                                        } else {
                                            "padding: 10px 16px; display: flex; align-items: center; justify-content: space-between; background: transparent; border-left: 3px solid transparent; cursor: pointer;"
                                        },
                                        onmouseenter: move |_| selected_idx.set(idx),
                                        onclick: move |_| run_palette_action(action_clone.clone(), is_open, store_click.clone()),

                                        div { style: "display: flex; align-items: center; gap: 12px;",
                                            div { style: "color: var(--accent);",
                                                IconSvg { name: action.icon, size: 16 }
                                            }
                                            div {
                                                div { style: "font-size: 12px; font-weight: 600; color: var(--text);", "{action.title}" }
                                                if let Some(sub) = &action.subtitle {
                                                    div { class: "mono", style: "font-size: 10px; color: var(--muted);", "{sub}" }
                                                }
                                            }
                                        }

                                        span { class: "tag mono", style: "font-size: 9px;", "{action.category}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Bottom strip hints
                div {
                    style: "display: flex; align-items: center; justify-content: space-between; padding: 8px 16px; background: var(--bg); border-top: 1px solid var(--border); font-size: 10px; color: var(--muted);",
                    div { style: "display: flex; gap: 12px;",
                        span { "↑↓ to navigate" }
                        span { "↵ to execute" }
                        span { "esc to close" }
                    }
                    span { class: "mono", "Ctrl+K / Cmd+K" }
                }
            }
        }
    }
}

fn run_palette_action(action: PaletteAction, mut is_open: Signal<bool>, mut store: Store) {
    is_open.set(false);

    if action.id.starts_with("nav:") {
        match action.id.as_str() {
            "nav:overview" => store.view.set(View::Overview),
            "nav:mods" => store.view.set(View::Mods),
            "nav:skins" => store.view.set(View::Skins),
            "nav:loadouts" => store.view.set(View::Loadouts),
            "nav:settings" => store.view.set(View::Settings),
            _ => {}
        }
    } else if let Some(gid) = action.id.strip_prefix("game:") {
        store.selected.set(Some(gid.to_string()));
        store.view.set(View::Overview);
    } else if action.id == "sys:panic" {
        store.panic.set(true);
        let core = store.core.clone();
        spawn(async move {
            let _ = core.trigger_panic().await;
        });
    } else if let Some(game) = store.selected_game() {
        let core = store.core.clone();
        let mut s = store.clone();
        match action.id.as_str() {
            "act:launch" => {
                let exe_path = game.exe_path.to_string_lossy().to_string();
                let gid = game.id.clone();
                spawn(async move {
                    s.toast(ToastKind::Info, format!("Launching {}...", game.name));
                    let _ = core.deploy_mods(exe_path, false).await;
                    if let Err(e) = core.launch_game(gid).await {
                        s.toast(ToastKind::Error, e);
                    }
                });
            }
            "act:hotreload" => {
                let exe_path = game.exe_path.to_string_lossy().to_string();
                spawn(async move {
                    match core.redeploy_mods(exe_path).await {
                        Ok(_) => s.toast(ToastKind::Success, "Mods redeployed & F10 sent"),
                        Err(e) => s.toast(ToastKind::Error, e),
                    }
                });
            }
            "act:shuffle" => {
                let gid = game.id.clone();
                spawn(async move {
                    match core.shuffle_character_skins(gid).await {
                        Ok(c) => s.toast(
                            ToastKind::Success,
                            format!("Shuffled {c} character skin(s)"),
                        ),
                        Err(e) => s.toast(ToastKind::Error, e),
                    }
                });
            }
            "act:wish" => {
                let gid = game.id.clone();
                spawn(async move {
                    match core.extract_wish_url(gid).await {
                        Ok(url) => {
                            let _ = document::eval(&format!(
                                "navigator.clipboard.writeText({:?})",
                                url
                            ));
                            s.toast(ToastKind::Success, "Gacha history URL copied to clipboard");
                        }
                        Err(e) => s.toast(ToastKind::Error, e),
                    }
                });
            }
            "act:audit" => {
                let gid = game.id.clone();
                spawn(async move {
                    match core.run_graphics_audit(gid).await {
                        Ok(warnings) => {
                            if warnings.is_empty() {
                                s.toast(
                                    ToastKind::Success,
                                    "Audit clean: No graphics or config issues found",
                                );
                            } else {
                                s.toast(
                                    ToastKind::Info,
                                    format!("Audit flagged {} recommendation(s)", warnings.len()),
                                );
                            }
                        }
                        Err(e) => s.toast(ToastKind::Error, e),
                    }
                });
            }
            "act:updates" => {
                let gid = game.id.clone();
                spawn(async move {
                    s.toast(ToastKind::Info, "Checking for mod updates on GameBanana...");
                    match core.check_mod_updates(gid).await {
                        Ok(ups) => {
                            if ups.is_empty() {
                                s.toast(ToastKind::Success, "All mods are up-to-date");
                            } else {
                                s.toast(
                                    ToastKind::Info,
                                    format!("Found {} mod update(s) available", ups.len()),
                                );
                            }
                        }
                        Err(e) => s.toast(ToastKind::Error, e),
                    }
                });
            }
            _ => {}
        }
    }
}
