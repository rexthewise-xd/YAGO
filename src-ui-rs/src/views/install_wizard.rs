use dioxus::prelude::*;
use sync::protocol::ManifestCategory;
use yago_app::GameConfigUpdate;

use crate::components::Modal;
use crate::icons::{IconName, IconSvg};
use crate::picker::{format_size, pick_folder};
use crate::state::{Store, ToastKind};

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Path,
    Categories,
    Confirm,
}

#[component]
pub fn InstallWizard(game_id: String, name: String) -> Element {
    let store = use_context::<Store>();
    let mut step = use_signal(|| Step::Path);
    let mut busy = use_signal(|| false);
    let mut categories = use_signal(Vec::<ManifestCategory>::new);
    let mut selected_ids = use_signal(Vec::<String>::new);

    let settings = (store.settings)();
    let default_path = if settings.default_games_path.as_os_str().is_empty() {
        String::new()
    } else {
        settings
            .default_games_path
            .join(&name)
            .to_string_lossy()
            .to_string()
    };
    let mut install_path = use_signal(|| default_path);

    let mut close_store = store.clone();
    let mut path_store = store.clone();
    let begin_store = store.clone();

    let gid = game_id.clone();
    let path_name = name.clone();
    let browse_name = name.clone();
    let begin_name = name.clone();

    let mut next_path = move |_| {
        let path = (install_path)();
        if path.trim().is_empty() {
            path_store.toast(ToastKind::Error, "Specify an installation path first.");
            return;
        }
        busy.set(true);
        let core = path_store.core.clone();
        let mut s = path_store.clone();
        let gid = gid.clone();
        let name = path_name.clone();
        spawn(async move {
            let result = async {
                let id = core.initialize_remote_game(gid.clone()).await?;
                let opts = core.get_install_options(id.clone()).await?;
                Ok::<_, String>((id, opts))
            }
            .await;
            busy.set(false);
            match result {
                Ok((id, opts)) => {
                    selected_ids.set(opts.iter().map(|c| c.id.clone()).collect());
                    categories.set(opts);
                    s.selected.set(Some(id));
                    step.set(Step::Categories);
                }
                Err(e) => {
                    s.toast(ToastKind::Error, format!("Failed to fetch {name} manifest: {e}"))
                }
            }
        });
    };

    let begin_gid = game_id.clone();
    let mut begin = move |_| {
        busy.set(true);
        let core = begin_store.core.clone();
        let mut s = begin_store.clone();
        let path = (install_path)();
        let ids = (selected_ids)();
        let gid = begin_gid.clone();
        let name = begin_name.clone();
        spawn(async move {
            let result = async {
                let update = GameConfigUpdate {
                    install_path: Some(path),
                    ..Default::default()
                };
                core.update_game_config(gid.clone(), update).await?;
                core.start_game_download(gid.clone(), ids).await
            }
            .await;
            busy.set(false);
            match result {
                Ok(()) => {
                    s.toast(ToastKind::Success, format!("{name} queued for installation"));
                    s.overlay.set(None);
                }
                Err(e) => {
                    s.toast(ToastKind::Error, format!("Failed to start installation: {e}"))
                }
            }
        });
    };

    let current = (step)();
    let cats = (categories)();
    let sel = (selected_ids)();
    let total: u64 = cats
        .iter()
        .filter(|c| sel.contains(&c.id))
        .map(|c| c.size)
        .sum();
    let path_value = (install_path)();
    let is_busy = (busy)();

    let step_label = match current {
        Step::Path => "01 PATH",
        Step::Categories => "02 CONTENT",
        Step::Confirm => "03 READY",
    };

    rsx! {
        Modal {
            title: format!("INSTALL {name}"),
            subtitle: step_label.to_string(),
            onclose: move |_| close_store.overlay.set(None),

            div { class: "wizard-steps mono",
                for (i, label) in ["PATH", "CONTENT", "READY"].iter().enumerate() {
                    span {
                        key: "{label}",
                        class: if i == current as usize { "wizard-step active" } else { "wizard-step" },
                        "{i + 1:02} {label}"
                    }
                }
            }

            if current == Step::Path {
                div { class: "wizard-section",
                    p { class: "wizard-note",
                        "Select the directory where the game will be installed. YAGO creates the game sub-folder automatically."
                    }
                    label { class: "field-label mono", "INSTALLATION DIRECTORY" }
                    div { class: "path-row",
                        input {
                            class: "input",
                            r#type: "text",
                            value: "{path_value}",
                            placeholder: "/home/user/Games/{name}",
                            oninput: move |evt| install_path.set(evt.value()),
                        }
                        button {
                            class: "btn",
                            title: "Browse",
                            onclick: move |_| {
                                let title = format!("Select installation folder for {browse_name}");
                                spawn(async move {
                                    if let Some(p) = pick_folder(&title).await {
                                        install_path.set(p.to_string_lossy().to_string());
                                    }
                                });
                            },
                            IconSvg { name: IconName::Folder, size: 15 }
                        }
                    }
                }
            }

            if current == Step::Categories {
                div { class: "wizard-section",
                    div { class: "wizard-cat-head",
                        span { class: "field-label mono", "SELECTIVE INSTALLATION" }
                        span { class: "mono wizard-total", "{format_size(total)} SELECTED" }
                    }
                    div { class: "cat-list",
                        for cat in cats.iter() {
                            CategoryRow {
                                key: "{cat.id}",
                                cat: cat.clone(),
                                selected: sel.contains(&cat.id),
                                ontoggle: move |id: String| {
                                    let mut ids = selected_ids.write();
                                    if ids.contains(&id) {
                                        ids.retain(|c| c != &id);
                                    } else {
                                        ids.push(id);
                                    }
                                },
                            }
                        }
                    }
                }
            }

            if current == Step::Confirm {
                div { class: "wizard-section wizard-confirm",
                    IconSvg { name: IconName::Check, size: 40 }
                    h3 { "READY FOR DEPLOYMENT" }
                    p { class: "wizard-note",
                        "YAGO will begin block-level reconstruction of {name}."
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "PATH" }
                        span { class: "kv-val", "{path_value}" }
                    }
                    div { class: "kv",
                        span { class: "kv-key mono", "CONTENT" }
                        span { class: "kv-val", "{sel.len()} CATEGORIES · {format_size(total)}" }
                    }
                }
            }

            div { class: "modal-actions",
                if current != Step::Path {
                    button {
                        class: "btn",
                        disabled: is_busy,
                        onclick: move |_| {
                            step.set(if current == Step::Categories { Step::Path } else { Step::Categories });
                        },
                        "BACK"
                    }
                }
                button {
                    class: "btn btn-accent",
                    disabled: is_busy,
                    onclick: move |evt| match current {
                        Step::Path => next_path(evt),
                        Step::Categories => step.set(Step::Confirm),
                        Step::Confirm => begin(evt),
                    },
                    if is_busy {
                        "FETCHING MANIFEST…"
                    } else if current == Step::Confirm {
                        "BEGIN INSTALLATION"
                    } else {
                        "NEXT"
                    }
                    IconSvg { name: IconName::ChevronRight, size: 14 }
                }
            }
        }
    }
}

#[component]
fn CategoryRow(cat: ManifestCategory, selected: bool, ontoggle: EventHandler<String>) -> Element {
    let id = cat.id.clone();
    let required = cat.is_required;
    rsx! {
        button {
            class: if selected { "cat-row selected" } else { "cat-row" },
            disabled: required,
            onclick: move |_| ontoggle.call(id.clone()),
            div { class: "cat-check",
                if selected || required {
                    IconSvg { name: IconName::Check, size: 12 }
                }
            }
            div { class: "cat-text",
                span { class: "cat-name", "{cat.name}" }
                span { class: "cat-meta mono",
                    if required { "CORE" } else { "OPTIONAL" }
                    " · {format_size(cat.size)}"
                }
            }
        }
    }
}