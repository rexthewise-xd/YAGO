use std::path::PathBuf;

use dioxus::prelude::*;
use storage::models::ImportCandidate;

use crate::art::encode_asset_url;
use crate::components::Modal;
use crate::icons::{IconName, IconSvg};

fn topology_class(topology: &str) -> &'static str {
    match topology {
        "Merged" => "topology topology-purple",
        "Standard" => "topology topology-blue",
        _ => "topology",
    }
}

fn candidate_thumb(c: &ImportCandidate) -> Option<String> {
    let preview = c.preview_image.as_deref()?.trim();
    if preview.is_empty() {
        return None;
    }
    let path = PathBuf::from(preview);
    let resolved = if path.is_absolute() {
        path
    } else {
        c.original_path.join(preview)
    };
    Some(encode_asset_url(&resolved))
}

#[component]
pub fn ImportStaging(
    candidates: Vec<ImportCandidate>,
    onconfirm: EventHandler<Vec<ImportCandidate>>,
    onclose: EventHandler<()>,
) -> Element {
    let mut items = use_signal(|| candidates.clone());
    let mut editing = use_signal(|| None::<usize>);

    let list = (items)();
    let enabled_count = list.iter().filter(|c| c.initial_state).count();
    let disabled_count = list.len() - enabled_count;
    let edit_idx = (editing)();

    let commit_items = items;
    let quick_items = items;
    let close_store = onclose;

    rsx! {
        Modal {
            title: "MIGRATION GATEWAY".to_string(),
            subtitle: Some(format!("FOUND {} MODS TO IMPORT", list.len())),
            wide: true,
            onclose: move |_| close_store.call(()),

            div { class: "staging-list",
                for (idx , c) in list.iter().enumerate() {
                    div {
                        key: "{idx}",
                        class: if c.initial_state { "staging-row" } else { "staging-row staging-off" },

                        button {
                            class: if c.initial_state { "switch on" } else { "switch" },
                            title: "Toggle import",
                            onclick: move |_| {
                                let mut list = items.write();
                                if let Some(row) = list.get_mut(idx) {
                                    row.initial_state = !row.initial_state;
                                }
                            },
                            span { class: "switch-knob" }
                        }

                        div { class: "staging-thumb",
                            if let Some(thumb) = candidate_thumb(c) {
                                img { src: "{thumb}", alt: "" }
                            } else {
                                IconSvg { name: IconName::File, size: 22 }
                            }
                        }

                        div { class: "staging-info",
                            div { class: "staging-name-row",
                                if edit_idx == Some(idx) {
                                    input {
                                        class: "input staging-name-input",
                                        r#type: "text",
                                        value: "{c.suggested_name}",
                                        autofocus: true,
                                        oninput: move |evt| {
                                            let mut list = items.write();
                                            if let Some(row) = list.get_mut(idx) {
                                                row.suggested_name = evt.value();
                                            }
                                        },
                                        onblur: move |_| editing.set(None),
                                        onkeydown: move |evt| {
                                            if evt.key() == Key::Enter {
                                                editing.set(None);
                                            }
                                        },
                                    }
                                } else {
                                    span {
                                        class: "staging-name",
                                        onclick: move |_| editing.set(Some(idx)),
                                        "{c.suggested_name}"
                                    }
                                }
                                span { class: topology_class(&c.detected_topology), "{c.detected_topology}" }
                            }
                            span { class: "staging-path mono", title: "{c.original_path.display()}", "{c.original_path.display()}" }
                        }

                        div { class: "staging-warnings",
                            for (widx , w) in c.warnings.iter().enumerate() {
                                span {
                                    key: "{widx}",
                                    class: match w.level.as_str() {
                                        "Critical" => "staging-warn warn-danger",
                                        "Warning" => "staging-warn warn-accent",
                                        _ => "staging-warn",
                                    },
                                    IconSvg { name: IconName::Warning, size: 11 }
                                    "{w.message}"
                                }
                            }
                        }

                        button {
                            class: "icon-btn staging-remove",
                            title: "Remove",
                            onclick: move |_| {
                                items.write().remove(idx);
                                editing.set(None);
                            },
                            IconSvg { name: IconName::Trash, size: 15 }
                        }
                    }
                }
            }

            div { class: "staging-foot",
                div { class: "staging-counts mono",
                    span { span { class: "dot ok" } "{enabled_count} ENABLED" }
                    span { span { class: "dot" } "{disabled_count} DISABLED" }
                }
                div { class: "staging-foot-actions",
                    button {
                        class: "btn-link",
                        onclick: move |_| {
                            let trusted: Vec<ImportCandidate> = quick_items
                                .read()
                                .iter()
                                .filter(|c| c.warnings.is_empty())
                                .cloned()
                                .collect();
                            onconfirm.call(trusted);
                        },
                        IconSvg { name: IconName::Deploy, size: 13 }
                        "QUICK IMPORT (TRUST YAGO)"
                    }
                    button {
                        class: "btn btn-accent",
                        onclick: move |_| {
                            onconfirm.call(commit_items.read().clone());
                        },
                        IconSvg { name: IconName::Check, size: 15 }
                        "COMMIT TO LIBRARY"
                    }
                }
            }
        }
    }
}