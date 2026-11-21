use dioxus::prelude::*;

use crate::icons::{IconName, IconSvg};
use crate::state::{Store, TaskState};

#[component]
pub fn ActivityPanel() -> Element {
    let store = use_context::<Store>();
    let downloads = (store.downloads)();
    let tasks = (store.tasks)();
    let active: Vec<_> = tasks
        .iter()
        .filter(|t| t.state == TaskState::Running)
        .cloned()
        .collect();
    let recent: Vec<_> = tasks
        .iter()
        .filter(|t| t.state != TaskState::Running)
        .rev()
        .take(6)
        .cloned()
        .collect();
    let ids: Vec<String> = downloads.keys().cloned().collect();

    rsx! {
        aside { class: "activity",
            div { class: "activity-heading", "ACTIVITY" }

            if ids.is_empty() && active.is_empty() && recent.is_empty() {
                div { class: "activity-empty",
                    IconSvg { name: IconName::Check, size: 20 }
                    span { "All quiet" }
                }
            }

            for game_id in ids.iter() {
                DownloadRow { key: "{game_id}", game_id: game_id.clone() }
            }

            for task in active.iter() {
                div { class: "task-row", key: "{task.id}",
                    span { class: "pulse-dot" }
                    div { class: "task-text",
                        span { class: "task-label", "{task.label}" }
                        span { class: "task-detail", "{task.detail}" }
                    }
                }
            }

            if !recent.is_empty() {
                div { class: "activity-subheading", "RECENT" }
                for task in recent.iter() {
                    div { class: "task-row", key: "{task.id}",
                        span {
                            class: match task.state {
                                TaskState::Done => "status-dot ok",
                                TaskState::Failed => "status-dot bad",
                                TaskState::Running => "status-dot busy",
                            }
                        }
                        div { class: "task-text",
                            span { class: "task-label", "{task.label}" }
                            span { class: "task-detail", "{task.detail}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DownloadRow(game_id: String) -> Element {
    let store = use_context::<Store>();
    let downloads = (store.downloads)();
    let Some(progress) = downloads.get(&game_id).cloned() else {
        return rsx! {};
    };
    let name = store
        .game(&game_id)
        .map(|g| g.short_name)
        .unwrap_or_else(|| game_id.clone());
    let pct = progress.percentage.clamp(0.0, 100.0);
    let speed = format_speed(progress.speed_bps);
    let eta = format_eta(progress.eta_secs);

    let pause_store = store.clone();
    let pause_id = game_id.clone();
    let resume_store = store.clone();
    let resume_id = game_id.clone();

    rsx! {
        div { class: "download-row",
            div { class: "download-head",
                span { class: "task-label", "{name}" }
                span { class: "mono", "{pct:.1}%" }
            }
            div { class: "progress-track",
                div { class: "progress-fill", style: "width: {pct}%" }
            }
            div { class: "download-meta mono",
                span { "{speed}" }
                span { "ETA {eta}" }
            }
            div { class: "download-controls",
                button {
                    class: "mini-btn",
                    title: "Pause",
                    onclick: move |_| {
                        let core = pause_store.core.clone();
                        let gid = pause_id.clone();
                        pause_store.call(
                            async move { core.pause_game_download(gid).await },
                            |_, _| {},
                        );
                    },
                    IconSvg { name: IconName::Pause, size: 12 }
                }
                button {
                    class: "mini-btn",
                    title: "Resume",
                    onclick: move |_| {
                        let core = resume_store.core.clone();
                        let gid = resume_id.clone();
                        resume_store.call(
                            async move { core.resume_game_download(gid).await },
                            |_, _| {},
                        );
                    },
                    IconSvg { name: IconName::Launch, size: 12 }
                }
            }
        }
    }
}

pub fn format_speed(bps: u64) -> String {
    if bps >= 1_048_576 {
        format!("{:.1} MB/s", bps as f64 / 1_048_576.0)
    } else if bps >= 1024 {
        format!("{:.0} KB/s", bps as f64 / 1024.0)
    } else {
        format!("{} B/s", bps)
    }
}

pub fn format_eta(secs: u64) -> String {
    if secs == 0 {
        return "--:--".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}