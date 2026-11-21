use dioxus::prelude::*;

use crate::art::cover_url;
use crate::components::Modal;
use crate::icons::{IconName, IconSvg};
use crate::state::{Store, ToastKind};
use yago_app::GameConfigUpdate;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Official,
    Presets,
    Upload,
}

#[component]
pub fn CoverManagerModal(
    game_id: String,
    onclose: EventHandler<()>,
) -> Element {
    let store = use_context::<Store>();
    let mut active_tab = use_signal(|| Tab::Official);
    let mut custom_url = use_signal(String::new);
    let mut preview = use_signal(|| None::<String>);

    let Some(game) = store.game(&game_id) else {
        return rsx! { div {} };
    };

    let current_cover = cover_url(&store, &game);
    let game_name = game.name.clone();
    let tab = (active_tab)();
    let preview_img = (preview)();
    let display_img = preview_img.as_deref().unwrap_or(&current_cover);
    let has_preview = preview_img.is_some();

    let core = store.core.clone();
    let gid_bg = game_id.clone();
    let community = use_resource(move || {
        let core = core.clone();
        let gid = gid_bg.clone();
        async move {
            core.get_community_backgrounds(gid).await.unwrap_or_default()
        }
    });

    let community_imgs: Vec<String> = (community)().unwrap_or_default();
    let presets = (store.app_config)().preset_covers.clone();

    rsx! {
        Modal {
            title: "CUSTOMIZE ART".to_string(),
            subtitle: Some("VISUAL IDENTITY".to_string()),
            wide: true,
            onclose: move |_| onclose.call(()),

            div { class: "cover-layout",
                div { class: "cover-preview",
                    div { class: "cover-preview-bg",
                        img { src: "{display_img}", alt: "" }
                    }
                    div { class: "cover-preview-card",
                        img { src: "{display_img}", alt: "Preview" }
                        div { class: "cover-preview-name", "{game_name}" }
                    }
                    span { class: "cover-preview-label mono", "BACKGROUND PREVIEW" }
                }

                div { class: "cover-controls",
                    div { class: "cover-tabs",
                        button {
                            class: if tab == Tab::Official { "seg-btn active" } else { "seg-btn" },
                            onclick: move |_| active_tab.set(Tab::Official),
                            "OFFICIAL"
                        }
                        button {
                            class: if tab == Tab::Presets { "seg-btn active" } else { "seg-btn" },
                            onclick: move |_| active_tab.set(Tab::Presets),
                            "PRESETS"
                        }
                        button {
                            class: if tab == Tab::Upload { "seg-btn active" } else { "seg-btn" },
                            onclick: move |_| active_tab.set(Tab::Upload),
                            "UPLOAD / URL"
                        }
                    }

                    div { class: "cover-content",
                        if tab == Tab::Official {
                            if community_imgs.is_empty() {
                                div { class: "cover-empty",
                                    IconSvg { name: IconName::Grid, size: 40 }
                                    p { "No Wallpapers Found" }
                                }
                            } else {
                                div { class: "cover-grid",
                                    for (i, url) in community_imgs.iter().enumerate() {
                                        CoverThumb {
                                            key: "{i}",
                                            url: url.clone(),
                                            selected: preview_img.as_deref() == Some(url.as_str()),
                                            onpick: move |url: String| preview.set(Some(url)),
                                        }
                                    }
                                }
                            }
                        }
                        if tab == Tab::Presets {
                            div { class: "cover-grid",
                                for (i, url) in presets.iter().enumerate() {
                                    CoverThumb {
                                        key: "{i}",
                                        url: url.clone(),
                                        selected: preview_img.as_deref() == Some(url.as_str()),
                                        onpick: move |url: String| preview.set(Some(url)),
                                    }
                                }
                            }
                        }
                        if tab == Tab::Upload {
                            div { class: "cover-upload",
                                label { class: "field-label mono", "EXTERNAL ASSET URL" }
                                div { class: "cover-url-row",
                                    input {
                                        class: "input",
                                        r#type: "text",
                                        placeholder: "https://example.com/wallpaper.jpg",
                                        value: "{(custom_url)()}",
                                        oninput: move |evt| custom_url.set(evt.value()),
                                    }
                                    button {
                                        class: "btn",
                                        disabled: (custom_url)().is_empty(),
                                        onclick: move |_| {
                                            let val = (custom_url)();
                                            if !val.is_empty() {
                                                preview.set(Some(val));
                                            }
                                        },
                                        IconSvg { name: IconName::Image, size: 18 }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "cover-actions",
                        button {
                            class: "btn-link danger",
                            onclick: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |_| {
                                    let core = store.core.clone();
                                    let gid = gid.clone();
                                    let fallback = format!("https://picsum.photos/seed/{gid}-landscape/1920/1080");
                                    let s = store.clone();
                                    s.call(
                                        async move {
                                            core.update_game_config(gid, GameConfigUpdate {
                                                cover_image: Some(fallback),
                                                ..Default::default()
                                            }).await
                                        },
                                        |s, _| s.toast(ToastKind::Success, "Cover reset"),
                                    );
                                    onclose.call(());
                                }
                            },
                            IconSvg { name: IconName::Trash, size: 14 }
                            "RESET DEFAULT"
                        }
                        div { class: "cover-actions-right",
                            button {
                                class: "btn",
                                onclick: move |_| onclose.call(()),
                                "DISCARD"
                            }
                            button {
                                class: "btn btn-accent",
                                disabled: !has_preview,
                                onclick: {
                                    let store = store.clone();
                                    let gid = game_id.clone();
                                    move |_| {
                                        if let Some(url) = (preview)() {
                                            let core = store.core.clone();
                                            let gid = gid.clone();
                                            let s = store.clone();
                                            s.call(
                                                async move {
                                                    core.update_game_config(gid, GameConfigUpdate {
                                                        cover_image: Some(url),
                                                        ..Default::default()
                                                    }).await
                                                },
                                                |s, _| s.toast(ToastKind::Success, "Cover updated"),
                                            );
                                            onclose.call(());
                                        }
                                    }
                                },
                                IconSvg { name: IconName::Check, size: 16 }
                                "APPLY CHANGES"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CoverThumb(
    url: String,
    selected: bool,
    onpick: EventHandler<String>,
) -> Element {
    let u = url.clone();
    rsx! {
        button {
            class: if selected { "cover-thumb selected" } else { "cover-thumb" },
            onclick: move |_| onpick.call(u.clone()),
            img { src: "{url}", alt: "", loading: "lazy" }
            if selected {
                div { class: "cover-thumb-check",
                    IconSvg { name: IconName::Check, size: 16 }
                }
            }
        }
    }
}
