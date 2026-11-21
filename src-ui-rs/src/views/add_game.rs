use dioxus::prelude::*;
use yago_app::{IdentifiedGame, RemoteCatalogEntry};

use crate::art::art_url;
use crate::components::Modal;
use crate::icons::{IconName, IconSvg};
use crate::picker::{format_size, pick_folder};
use crate::state::{Overlay, Store, ToastKind};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Local,
    Cloud,
}

#[component]
pub fn AddGameModal() -> Element {
    let store = use_context::<Store>();
    let mut tab = use_signal(|| Tab::Local);
    let mut scanning = use_signal(|| false);
    let mut results = use_signal(Vec::<IdentifiedGame>::new);
    let mut catalog = use_signal(Vec::<RemoteCatalogEntry>::new);
    let mut catalog_loaded = use_signal(|| false);
    let mut catalog_busy = use_signal(|| false);

    let mut close_store = store.clone();
    let scan_store = store.clone();
    let browse_store = store.clone();
    let cloud_store = store.clone();

    let run_scan = move |_| {
        scanning.set(true);
        let core = scan_store.core.clone();
        let mut s = scan_store.clone();
        spawn(async move {
            let found = async {
                let discovered = core.scan_for_games().await?;
                let mut identified = Vec::new();
                for d in discovered {
                    if let Ok(g) = core
                        .identify_game(d.path.to_string_lossy().to_string())
                        .await
                    {
                        identified.push(g);
                    }
                }
                Ok::<_, String>(identified)
            }
            .await;
            scanning.set(false);
            match found {
                Ok(found) => {
                    let mut list = results.write();
                    for g in found {
                        if !list.iter().any(|e| e.install_path == g.install_path) {
                            list.push(g);
                        }
                    }
                    if list.is_empty() {
                        s.toast(ToastKind::Info, "No supported titles found in common paths.");
                    }
                }
                Err(e) => s.toast(ToastKind::Error, format!("Scan failed: {e}")),
            }
        });
    };

    let run_browse = move |_| {
        let core = browse_store.core.clone();
        let mut s = browse_store.clone();
        spawn(async move {
            let Some(path) = pick_folder("Select game folder").await else {
                return;
            };
            let path_str = path.to_string_lossy().to_string();
            match core.identify_game(path_str.clone()).await {
                Ok(g) => {
                    let mut list = results.write();
                    if !list.iter().any(|e| e.install_path == g.install_path) {
                        list.push(g);
                    }
                }
                Err(e) => s.toast(ToastKind::Error, format!("Not a supported game: {e}")),
            }
        });
    };

    let load_cloud = move |_| {
        tab.set(Tab::Cloud);
        if (catalog_loaded)() {
            return;
        }
        catalog_busy.set(true);
        let core = cloud_store.core.clone();
        let mut s = cloud_store.clone();
        spawn(async move {
            let result = core.get_remote_catalog().await;
            catalog_busy.set(false);
            match result {
                Ok(entries) => {
                    catalog.set(entries);
                    catalog_loaded.set(true);
                }
                Err(e) => s.toast(ToastKind::Error, format!("Failed to load catalog: {e}")),
            }
        });
    };

    let current_tab = (tab)();
    let is_scanning = (scanning)();
    let found = (results)();
    let entries = (catalog)();
    let cloud_busy = (catalog_busy)();

    rsx! {
        Modal {
            title: "ADD GAME".to_string(),
            subtitle: "LOCAL LIBRARY OR REMOTE CATALOG".to_string(),
            wide: true,
            onclose: move |_| close_store.overlay.set(None),

            div { class: "tab-row",
                button {
                    class: if current_tab == Tab::Local { "tab active" } else { "tab" },
                    onclick: move |_| tab.set(Tab::Local),
                    IconSvg { name: IconName::Folder, size: 14 }
                    span { class: "tab-label", "LOCAL" }
                }
                button {
                    class: if current_tab == Tab::Cloud { "tab active" } else { "tab" },
                    onclick: load_cloud,
                    IconSvg { name: IconName::Download, size: 14 }
                    span { class: "tab-label", "CLOUD" }
                }
            }

            if current_tab == Tab::Local {
                div { class: "wizard-section",
                    div { class: "add-actions",
                        button {
                            class: "btn btn-accent",
                            disabled: is_scanning,
                            onclick: run_scan,
                            IconSvg { name: IconName::Search, size: 15 }
                            if is_scanning { "SCANNING…" } else { "SCAN COMMON PATHS" }
                        }
                        button {
                            class: "btn",
                            onclick: run_browse,
                            IconSvg { name: IconName::Folder, size: 15 }
                            "BROWSE FOLDER"
                        }
                    }

                    if found.is_empty() {
                        p { class: "wizard-note",
                            "Scan detects installed titles in common locations. Browse picks a folder manually. Identified games appear below."
                        }
                    } else {
                        div { class: "found-list",
                            for g in found.iter() {
                                FoundRow { key: "{g.install_path}", game: g.clone() }
                            }
                        }
                    }
                }
            }

            if current_tab == Tab::Cloud {
                div { class: "wizard-section",
                    if cloud_busy {
                        p { class: "wizard-note mono", "LOADING CATALOG…" }
                    } else if entries.is_empty() {
                        p { class: "wizard-note", "No remote titles available." }
                    } else {
                        div { class: "catalog-grid",
                            for entry in entries.iter() {
                                CatalogCard { key: "{entry.template.id}", entry: entry.clone() }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FoundRow(game: IdentifiedGame) -> Element {
    let store = use_context::<Store>();
    let icon = art_url(&store, &game.icon);
    let path = game.install_path.clone();
    let add_path = game.install_path.clone();
    let name = game.name.clone();

    rsx! {
        div { class: "found-row",
            img { class: "found-icon", src: "{icon}", alt: "" }
            div { class: "found-text",
                span { class: "found-name", "{game.name}" }
                span { class: "found-path mono", title: "{path}", "{path}" }
                span { class: "found-meta mono",
                    "v{game.version} · {game.size} · {game.exe_name}"
                }
            }
            button {
                class: "btn btn-accent",
                onclick: move |_| {
                    let core = store.core.clone();
                    let s = store.clone();
                    let path = add_path.clone();
                    let name = name.clone();
                    s.call(
                        async move { core.add_game(path).await },
                        move |s, id| {
                            s.toast(ToastKind::Success, format!("{name} added to library"));
                            s.selected.set(Some(id));
                            s.overlay.set(None);
                        },
                    );
                },
                IconSvg { name: IconName::Add, size: 14 }
                "ADD"
            }
        }
    }
}

#[component]
fn CatalogCard(entry: RemoteCatalogEntry) -> Element {
    let mut store = use_context::<Store>();
    let t = &entry.template;
    let cover = art_url(&store, &t.cover_image);
    let accent = if t.accent_color.is_empty() {
        "#22d3ee".to_string()
    } else {
        t.accent_color.clone()
    };
    let gid = t.id.clone();
    let name = t.name.clone();
    let remote_meta = entry.remote_info.as_ref().map(|r| {
        format!("v{} · {}", r.version, format_size(r.total_size))
    });

    rsx! {
        div { class: "catalog-card", style: "--tile-accent: {accent}",
            img { class: "catalog-cover", src: "{cover}", alt: "" }
            div { class: "catalog-body",
                span { class: "catalog-name", "{t.name}" }
                span { class: "catalog-dev mono", "{t.developer}" }
                if let Some(meta) = remote_meta {
                    span { class: "catalog-meta mono", "{meta}" }
                }
                button {
                    class: "btn btn-accent",
                    onclick: move |_| {
                        store.overlay.set(Some(Overlay::Install {
                            game_id: gid.clone(),
                            name: name.clone(),
                        }));
                    },
                    IconSvg { name: IconName::Download, size: 14 }
                    "INSTALL"
                }
            }
        }
    }
}