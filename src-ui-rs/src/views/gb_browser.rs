use dioxus::prelude::*;
use yago_app::GbModSummary;

use crate::components::Modal;
use crate::icons::{IconName, IconSvg};
use crate::state::{Store, ToastKind};

#[component]
pub fn GameBananaBrowserModal(game_id: String, onclose: EventHandler<()>) -> Element {
    let store = use_context::<Store>();
    let mut search_input = use_signal(String::new);
    let mut active_query = use_signal(String::new);
    let mut current_page = use_signal(|| 1u32);
    let mut include_nsfw = use_signal(|| false);
    let mut installing_id = use_signal(|| None::<u64>);
    let mut is_loading = use_signal(|| false);
    let mut results = use_signal(Vec::<GbModSummary>::new);
    let mut direct_installing = use_signal(|| false);

    let stream_safe = (store.settings)().stream_safe;

    // Load results on change of active_query, current_page, include_nsfw
    let gid = game_id.clone();
    let q = (active_query)();
    let p = (current_page)();
    let nsfw = !stream_safe && (include_nsfw)();
    let effect_store = store.clone();

    use_effect(move || {
        let core = effect_store.core.clone();
        let gid = gid.clone();
        let q = q.clone();
        let mut s = effect_store.clone();
        spawn(async move {
            is_loading.set(true);
            match core.browse_gamebanana(gid, q, p, nsfw).await {
                Ok(items) => results.set(items),
                Err(e) => {
                    s.toast(ToastKind::Error, e);
                    results.set(Vec::new());
                }
            }
            is_loading.set(false);
        });
    });

    let mod_list = (results)();
    let loading = (is_loading)();
    let installing = (installing_id)();
    let input_val = (search_input)();
    let is_direct_url =
        input_val.contains("gamebanana.com/mods/") || input_val.starts_with("yago://");
    let nsfw_btn_label = if (include_nsfw)() {
        "NSFW: ON"
    } else {
        "NSFW: OFF"
    };

    rsx! {
        Modal {
            title: "GAMEBANANA BROWSER".to_string(),
            subtitle: Some("Search and 1-Click install mods directly into your library".to_string()),
            wide: true,
            onclose: move |_| onclose.call(()),

            div { class: "gb-browser-bar",
                div { style: "display: flex; gap: 8px; flex: 1;",
                    input {
                        class: "input",
                        style: "flex: 1;",
                        placeholder: "Search mods or paste GameBanana / yago:// URL...",
                        value: "{input_val}",
                        oninput: move |e| search_input.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key() == Key::Enter {
                                active_query.set(search_input().trim().to_string());
                                current_page.set(1);
                            }
                        },
                    }
                    button {
                        class: "btn btn-accent",
                        onclick: move |_| {
                            active_query.set(search_input().trim().to_string());
                            current_page.set(1);
                        },
                        IconSvg { name: IconName::Search, size: 14 }
                        "SEARCH"
                    }
                }

                if !stream_safe {
                    button {
                        class: if (include_nsfw)() { "btn btn-sm btn-accent" } else { "btn btn-sm" },
                        onclick: move |_| {
                            let curr = (include_nsfw)();
                            include_nsfw.set(!curr);
                            current_page.set(1);
                        },
                        "{nsfw_btn_label}"
                    }
                }
            }

            if is_direct_url {
                div {
                    style: "padding: 8px 12px; background: rgba(34, 211, 238, 0.1); border: 1px solid var(--accent); border-radius: 4px; display: flex; align-items: center; justify-content: space-between;",
                    div { style: "font-size: 11px; color: var(--accent);",
                        "Direct 1-Click Install URL detected in input"
                    }
                    button {
                        class: "btn btn-sm btn-accent",
                        disabled: (direct_installing)(),
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            let url = input_val.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let url = url.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    direct_installing.set(true);
                                    s.toast(ToastKind::Info, "Downloading and installing mod...");
                                    match core.handle_deep_link(gid, url).await {
                                        Ok(rec) => {
                                            s.toast(ToastKind::Success, format!("Installed {}", rec.meta.name));
                                            search_input.set(String::new());
                                        }
                                        Err(e) => s.toast(ToastKind::Error, format!("Install failed: {e}")),
                                    }
                                    direct_installing.set(false);
                                });
                            }
                        },
                        if (direct_installing)() { "INSTALLING…" } else { "1-CLICK INSTALL" }
                    }
                }
            }

            if loading {
                div { style: "text-align: center; padding: 40px; color: var(--muted); font-size: 13px;",
                    "Loading mods from GameBanana..."
                }
            } else if mod_list.is_empty() {
                div { style: "text-align: center; padding: 40px; color: var(--muted); font-size: 13px;",
                    "No mods found. Try a different search query."
                }
            } else {
                div { class: "gb-grid",
                    for item in mod_list.iter() {
                        {
                            let m_id = item.id;
                            let m_name = item.name.clone();
                            let m_author = item.author.clone();
                            let m_thumb = item.thumbnail_url.clone();
                            let m_likes = item.like_count;
                            let m_views = item.view_count;
                            let is_curr_installing = installing == Some(m_id);

                            let store = store.clone();
                            let gid = game_id.clone();

                            rsx! {
                                div { key: "{m_id}", class: "gb-card",
                                    div { class: "gb-card-thumb",
                                        if let Some(t) = m_thumb {
                                            img { src: "{t}", alt: "{m_name}" }
                                        } else {
                                            div { class: "gb-thumb-placeholder",
                                                IconSvg { name: IconName::Image, size: 28 }
                                            }
                                        }
                                    }
                                    div { class: "gb-card-info",
                                        div { class: "gb-card-name", title: "{m_name}", "{m_name}" }
                                        div { class: "gb-card-author mono", "by {m_author}" }
                                        div { class: "gb-card-stats mono",
                                            span { "♥ {m_likes}" }
                                            span { "👁 {m_views}" }
                                        }
                                    }
                                    button {
                                        class: "btn btn-sm btn-accent gb-card-install",
                                        disabled: is_curr_installing,
                                        onclick: move |_| {
                                            let core = store.core.clone();
                                            let gid = gid.clone();
                                            let mut s = store.clone();
                                            spawn(async move {
                                                installing_id.set(Some(m_id));
                                                s.toast(ToastKind::Info, format!("Downloading mod {m_id}..."));
                                                match core.install_from_gamebanana(gid, m_id, None).await {
                                                    Ok(rec) => {
                                                        s.toast(ToastKind::Success, format!("Installed {}", rec.meta.name));
                                                    }
                                                    Err(e) => {
                                                        s.toast(ToastKind::Error, format!("Install failed: {e}"));
                                                    }
                                                }
                                                installing_id.set(None);
                                            });
                                        },
                                        if is_curr_installing { "INSTALLING…" } else { "INSTALL" }
                                    }
                                }
                            }
                        }
                    }
                }

                div { style: "display: flex; justify-content: space-between; align-items: center; padding-top: 10px;",
                    button {
                        class: "btn btn-sm",
                        disabled: p <= 1,
                        onclick: move |_| {
                            if (current_page)() > 1 {
                                current_page.set((current_page)() - 1);
                            }
                        },
                        "◀ PREV"
                    }
                    span { class: "mono", style: "font-size: 11px; color: var(--muted);", "PAGE {p}" }
                    button {
                        class: "btn btn-sm",
                        onclick: move |_| {
                            current_page.set((current_page)() + 1);
                        },
                        "NEXT ▶"
                    }
                }
            }
        }
    }
}
