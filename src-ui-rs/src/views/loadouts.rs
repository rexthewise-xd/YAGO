use dioxus::prelude::*;
use storage::exporter::export_profile_for_sharing;
use storage::models::Profile;
use uuid::Uuid;
use yago_app::ProfileUpdate;

use crate::components::{Confirm, Modal};
use crate::icons::{IconName, IconSvg};
use crate::picker::pick_folder;
use crate::state::{Store, ToastKind};

#[component]
pub fn LoadoutsView() -> Element {
    let store = use_context::<Store>();
    let mut selected_pid = use_signal(|| None::<String>);
    let mut show_create_modal = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut rename_target = use_signal(|| None::<(String, String)>); // (id, current_name)
    let mut rename_val = use_signal(String::new);
    let mut delete_target = use_signal(|| None::<(String, String)>); // (id, name)
    let mut export_code = use_signal(|| None::<(String, String)>); // (profile_name, code)
    let mut export_bundle_json = use_signal(|| None::<(String, String)>);
    let mut show_import_bundle_modal = use_signal(|| false);
    let mut import_json_input = use_signal(String::new);
    let mut import_bundle_report = use_signal(|| None::<yago_app::BundleImportReport>);

    let Some(game) = store.selected_game() else {
        return rsx! {
            div { class: "overview-empty",
                IconSvg { name: IconName::Profiles, size: 32 }
                h2 { "No title selected" }
                p { "Select a game to manage its profiles and loadouts." }
            }
        };
    };

    let game_id = game.id.clone();
    let db = store.db(&game_id);
    let profiles: Vec<Profile> = db
        .as_ref()
        .map(|d| {
            let mut p: Vec<Profile> = d.profiles.values().cloned().collect();
            p.sort_by(|a, b| a.added_at.cmp(&b.added_at));
            p
        })
        .unwrap_or_default();

    let active_id = game.active_profile_id.clone();
    let selected_id = (selected_pid)()
        .filter(|id| profiles.iter().any(|p| p.id.to_string() == *id))
        .unwrap_or_else(|| active_id.clone());

    let inspect_profile = profiles
        .iter()
        .find(|p| p.id.to_string() == selected_id)
        .cloned()
        .or_else(|| profiles.first().cloned());

    let inspect_pname = inspect_profile
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let has_inspect = inspect_profile.is_some();

    let profile_count = profiles.len();
    let active_profile_name = profiles
        .iter()
        .find(|p| p.id.to_string() == active_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Default".to_string());

    let mods = db.as_ref().map(|d| d.mods.clone()).unwrap_or_default();

    rsx! {
        div { class: "loadouts-view",
            div { class: "mods-header",
                div { class: "mods-title",
                    IconSvg { name: IconName::Profiles, size: 20 }
                    h1 { "LOADOUTS & PROFILES" }
                    span { class: "mods-count mono", "{profile_count}" }
                    span { class: "char-badge accent mono", "ACTIVE: {active_profile_name}" }
                }
                div { class: "mods-header-actions",
                    button {
                        class: "btn btn-accent",
                        onclick: move |_| {
                            new_name.set(String::new());
                            show_create_modal.set(true);
                        },
                        IconSvg { name: IconName::Add, size: 15 }
                        "NEW LOADOUT"
                    }
                    button {
                        class: "btn",
                        disabled: !has_inspect,
                        onclick: {
                            let store = store.clone();
                            let gid = game_id.clone();
                            let pid = selected_id.clone();
                            let pname = inspect_pname.clone();
                            move |_| {
                                let core = store.core.clone();
                                let gid = gid.clone();
                                let pid = pid.clone();
                                let pname = pname.clone();
                                let mut s = store.clone();
                                spawn(async move {
                                    match core.export_profile_bundle(gid, pid).await {
                                        Ok(json) => {
                                            let _ = document::eval(&format!("navigator.clipboard.writeText({:?})", json));
                                            export_bundle_json.set(Some((pname, json)));
                                            s.toast(ToastKind::Success, "Loadout bundle JSON copied to clipboard");
                                        }
                                        Err(e) => s.toast(ToastKind::Error, e),
                                    }
                                });
                            }
                        },
                        IconSvg { name: IconName::Copy, size: 14 }
                        "EXPORT JSON"
                    }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            import_json_input.set(String::new());
                            import_bundle_report.set(None);
                            show_import_bundle_modal.set(true);
                        },
                        IconSvg { name: IconName::Download, size: 14 }
                        "IMPORT JSON"
                    }
                }
            }

            div { class: "loadouts-body",
                div { class: "loadouts-list-col",
                    div { class: "loadouts-list",
                        for p in profiles.iter() {
                            {
                                let pid_str = p.id.to_string();
                                let is_active = pid_str == active_id;
                                let is_selected = pid_str == selected_id;
                                let p_clone = p.clone();
                                let p_name = p.name.clone();
                                let p_uuid = p.id;
                                let enabled_count = p.enabled_mod_ids.len();
                                let activate_store = store.clone();
                                let activate_gid = game_id.clone();
                                let activate_pid = pid_str.clone();
                                let clone_store = store.clone();
                                let clone_gid = game_id.clone();
                                let clone_name = format!("{} (Copy)", p_name);
                                let select_pid = pid_str.clone();
                                let rename_pid = pid_str.clone();
                                let rename_name = p_name.clone();
                                let delete_pid = pid_str.clone();
                                let delete_name = p_name.clone();
                                rsx! {
                                    div {
                                        key: "{pid_str}",
                                        class: {
                                            let mut c = String::from("loadout-card");
                                            if is_selected { c.push_str(" selected"); }
                                            if is_active { c.push_str(" active-loadout"); }
                                            c
                                        },
                                        onclick: move |_| selected_pid.set(Some(select_pid.clone())),

                                        div { class: "loadout-card-top",
                                            div { class: "loadout-card-info",
                                                div { class: "loadout-name-row",
                                                    span { class: "loadout-name", "{p.name}" }
                                                    if is_active {
                                                        span { class: "char-badge accent mono", "ACTIVE" }
                                                    }
                                                }
                                                span { class: "loadout-desc", "{p.description}" }
                                                div { class: "loadout-meta mono",
                                                    span { "{enabled_count} MODS ENABLED" }
                                                    if p.use_reshade { span { "· RESHADE" } }
                                                    if p.use_gamemode { span { "· GAMEMODE" } }
                                                    if p.use_gamescope { span { "· GAMESCOPE" } }
                                                }
                                            }
                                        }

                                        div { class: "loadout-card-actions",
                                            if !is_active {
                                                button {
                                                    class: "btn btn-sm btn-accent",
                                                    onclick: move |evt: Event<MouseData>| {
                                                        evt.stop_propagation();
                                                        let core = activate_store.core.clone();
                                                        let gid = activate_gid.clone();
                                                        let pid = activate_pid.clone();
                                                        let s = activate_store.clone();
                                                        s.call(
                                                            async move { core.switch_profile(gid, pid).await },
                                                            |s, _| s.toast(ToastKind::Success, "Profile activated"),
                                                        );
                                                    },
                                                    IconSvg { name: IconName::Launch, size: 12 }
                                                    "ACTIVATE"
                                                }
                                            }

                                            button {
                                                class: "btn btn-sm",
                                                title: "Duplicate profile",
                                                onclick: {
                                                    let clone_name = clone_name.clone();
                                                    move |evt: Event<MouseData>| {
                                                        evt.stop_propagation();
                                                        let core = clone_store.core.clone();
                                                        let gid = clone_gid.clone();
                                                        let name = clone_name.clone();
                                                        let s = clone_store.clone();
                                                        s.call(
                                                            async move { core.duplicate_profile(gid, p_uuid, name).await },
                                                            |s, _| s.toast(ToastKind::Success, "Profile duplicated"),
                                                        );
                                                    }
                                                },
                                                IconSvg { name: IconName::Copy, size: 13 }
                                                "CLONE"
                                            }

                                            button {
                                                class: "icon-btn",
                                                title: "Rename profile",
                                                onclick: move |evt: Event<MouseData>| {
                                                    evt.stop_propagation();
                                                    rename_val.set(rename_name.clone());
                                                    rename_target.set(Some((rename_pid.clone(), rename_name.clone())));
                                                },
                                                IconSvg { name: IconName::Edit, size: 14 }
                                            }

                                            button {
                                                class: "icon-btn",
                                                title: "Share profile code",
                                                onclick: {
                                                    let p = p_clone.clone();
                                                    move |evt: Event<MouseData>| {
                                                        evt.stop_propagation();
                                                        let code = export_profile_for_sharing(&p);
                                                        export_code.set(Some((p.name.clone(), code)));
                                                    }
                                                },
                                                IconSvg { name: IconName::Deploy, size: 14 }
                                            }

                                            if !is_active && profile_count > 1 {
                                                button {
                                                    class: "icon-btn icon-danger",
                                                    title: "Delete profile",
                                                    onclick: move |evt: Event<MouseData>| {
                                                        evt.stop_propagation();
                                                        delete_target.set(Some((delete_pid.clone(), delete_name.clone())));
                                                    },
                                                    IconSvg { name: IconName::Trash, size: 14 }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "loadouts-detail-col",
                    if let Some(profile) = inspect_profile {
                        ProfileDetailEditor {
                            key: "{profile.id}",
                            profile,
                            game_id: game_id.clone(),
                            mods: mods.clone(),
                        }
                    } else {
                        div { class: "inspector-empty",
                            IconSvg { name: IconName::Profiles, size: 48 }
                            h3 { "No Profile Selected" }
                            p { "Select or create a profile on the left." }
                        }
                    }
                }
            }

            if (show_create_modal)() {
                Modal {
                    title: "CREATE NEW LOADOUT".to_string(),
                    subtitle: Some("ISOLATED MOD COMBINATION".to_string()),
                    onclose: move |_| show_create_modal.set(false),

                    div { class: "field-group",
                        label { class: "field-label", "Loadout Name" }
                        input {
                            class: "input",
                            placeholder: "e.g., Performance, Abyss Run, Casual...",
                            value: "{(new_name)()}",
                            autofocus: true,
                            oninput: move |e| new_name.set(e.value()),
                            onkeydown: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |e: KeyboardEvent| {
                                    if e.key() == Key::Enter {
                                        let name = (new_name)().trim().to_string();
                                        if !name.is_empty() {
                                            let core = store.core.clone();
                                            let gid = gid.clone();
                                            let s = store.clone();
                                            s.call(
                                                async move { core.create_profile(gid, name).await },
                                                |s, _| {
                                                    s.toast(ToastKind::Success, "Profile created");
                                                },
                                            );
                                            show_create_modal.set(false);
                                        }
                                    }
                                }
                            },
                        }
                    }

                    div { class: "modal-actions",
                        button {
                            class: "btn",
                            onclick: move |_| show_create_modal.set(false),
                            "CANCEL"
                        }
                        button {
                            class: "btn btn-accent",
                            disabled: (new_name)().trim().is_empty(),
                            onclick: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                move |_| {
                                    let name = (new_name)().trim().to_string();
                                    if !name.is_empty() {
                                        let core = store.core.clone();
                                        let gid = gid.clone();
                                        let s = store.clone();
                                        s.call(
                                            async move { core.create_profile(gid, name).await },
                                            |s, _| {
                                                s.toast(ToastKind::Success, "Profile created");
                                            },
                                        );
                                        show_create_modal.set(false);
                                    }
                                }
                            },
                            IconSvg { name: IconName::Add, size: 14 }
                            "CREATE"
                        }
                    }
                }
            }

            if let Some((pid, _current)) = (rename_target)() {
                Modal {
                    title: "RENAME LOADOUT".to_string(),
                    subtitle: None,
                    onclose: move |_| rename_target.set(None),

                    div { class: "field-group",
                        label { class: "field-label", "New Name" }
                        input {
                            class: "input",
                            value: "{(rename_val)()}",
                            autofocus: true,
                            oninput: move |e| rename_val.set(e.value()),
                        }
                    }

                    div { class: "modal-actions",
                        button {
                            class: "btn",
                            onclick: move |_| rename_target.set(None),
                            "CANCEL"
                        }
                        button {
                            class: "btn btn-accent",
                            disabled: (rename_val)().trim().is_empty(),
                            onclick: {
                                let store = store.clone();
                                let gid = game_id.clone();
                                let pid = pid.clone();
                                move |_| {
                                    let name = (rename_val)().trim().to_string();
                                    if !name.is_empty() {
                                        let core = store.core.clone();
                                        let gid = gid.clone();
                                        let pid = pid.clone();
                                        let s = store.clone();
                                        s.call(
                                            async move { core.rename_profile(gid, pid, name).await },
                                            |s, _| s.toast(ToastKind::Success, "Profile renamed"),
                                        );
                                        rename_target.set(None);
                                    }
                                }
                            },
                            "SAVE"
                        }
                    }
                }
            }

            if let Some((del_id, del_name)) = (delete_target)() {
                Confirm {
                    title: "Delete Profile".to_string(),
                    message: format!("Are you sure you want to delete profile \"{del_name}\"? Its mod configurations and sandboxed state will be permanently removed."),
                    confirm_label: "Delete Profile".to_string(),
                    danger: true,
                    onconfirm: {
                        let store = store.clone();
                        let gid = game_id.clone();
                        let pid = del_id.clone();
                        move |_| {
                            let core = store.core.clone();
                            let gid = gid.clone();
                            let pid = pid.clone();
                            let s = store.clone();
                            s.call(
                                async move { core.delete_profile(gid, pid).await },
                                |s, _| s.toast(ToastKind::Success, "Profile deleted"),
                            );
                            delete_target.set(None);
                        }
                    },
                    oncancel: move |_| delete_target.set(None),
                }
            }

            if let Some((pname, code)) = (export_code)() {
                Modal {
                    title: format!("EXPORT: {pname}"),
                    subtitle: Some("PROFILE SHARING STRING".to_string()),
                    onclose: move |_| export_code.set(None),

                    p { class: "wizard-note", "Share this loadout code with other YAGO users to replicate your exact mod selection." }

                    div { class: "field-group",
                        textarea {
                            class: "input textarea mono",
                            readonly: true,
                            rows: 3,
                            value: "{code}",
                        }
                    }

                    div { class: "modal-actions",
                        button {
                            class: "btn btn-accent",
                            onclick: move |_| export_code.set(None),
                            "DONE"
                        }
                    }
                }
            }

            if let Some((pname, json)) = (export_bundle_json)() {
                Modal {
                    title: format!("EXPORT: {pname}"),
                    subtitle: Some("SHAREABLE LOADOUT BUNDLE JSON".to_string()),
                    onclose: move |_| export_bundle_json.set(None),

                    p { class: "wizard-note", "Share this complete loadout bundle with other players. It encodes all enabled mods, load order indices, and variant presets." }

                    div { class: "field-group",
                        textarea {
                            class: "input textarea mono",
                            readonly: true,
                            rows: 8,
                            value: "{json}",
                        }
                    }

                    div { class: "modal-actions",
                        button {
                            class: "btn btn-accent",
                            onclick: move |_| export_bundle_json.set(None),
                            "DONE"
                        }
                    }
                }
            }

            if (show_import_bundle_modal)() {
                Modal {
                    title: "IMPORT LOADOUT BUNDLE",
                    subtitle: Some("PASTE JSON BUNDLE".to_string()),
                    onclose: move |_| show_import_bundle_modal.set(false),

                    if let Some(rep) = (import_bundle_report)() {
                        div { style: "display: flex; flex-direction: column; gap: 8px;",
                            div { class: "wizard-note", "Bundle imported successfully!" }
                            div { class: "stat-row mono",
                                span { "{rep.matched_count} MODS MATCHED & ENABLED" }
                                span { "{rep.missing_mods.len()} MODS MISSING LOCALLY" }
                            }
                            if !rep.missing_mods.is_empty() {
                                div { style: "font-size: 11px; color: var(--warn); margin-top: 4px;",
                                    "Missing mods: "
                                    for (idx, m) in rep.missing_mods.iter().enumerate() {
                                        span { class: "mono", if idx > 0 { ", " } else { "" } "{m}" }
                                    }
                                }
                            }
                            div { class: "modal-actions",
                                button {
                                    class: "btn btn-accent",
                                    onclick: move |_| show_import_bundle_modal.set(false),
                                    "CLOSE"
                                }
                            }
                        }
                    } else {
                        p { class: "wizard-note", "Paste a Loadout Bundle JSON string below to create a new loadout profile matching your local mods." }

                        div { class: "field-group",
                            textarea {
                                class: "input textarea mono",
                                placeholder: "{{\n  \"version\": 1,\n  \"game_id\": \"...\"\n}}",
                                rows: 8,
                                value: "{(import_json_input)()}",
                                oninput: move |e| import_json_input.set(e.value()),
                            }
                        }

                        div { class: "modal-actions",
                            button {
                                class: "btn",
                                onclick: move |_| show_import_bundle_modal.set(false),
                                "CANCEL"
                            }
                            button {
                                class: "btn btn-accent",
                                disabled: (import_json_input)().trim().is_empty(),
                                onclick: {
                                    let store = store.clone();
                                    let gid = game_id.clone();
                                    move |_| {
                                        let core = store.core.clone();
                                        let gid = gid.clone();
                                        let json = (import_json_input)().trim().to_string();
                                        let mut s = store.clone();
                                        spawn(async move {
                                            match core.import_profile_bundle(gid, json).await {
                                                Ok(rep) => {
                                                    s.toast(ToastKind::Success, format!("Imported profile with {} matched mod(s)", rep.matched_count));
                                                    import_bundle_report.set(Some(rep));
                                                }
                                                Err(e) => s.toast(ToastKind::Error, e),
                                            }
                                        });
                                    }
                                },
                                "IMPORT"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── ProfileDetailEditor ─────────────────────────────────────────

#[component]
fn ProfileDetailEditor(
    profile: Profile,
    game_id: String,
    mods: std::collections::HashMap<Uuid, storage::models::ModRecord>,
) -> Element {
    let store = use_context::<Store>();

    let mut desc = use_signal(|| profile.description.clone());
    let mut launch_args = use_signal(|| profile.launch_args.join(" "));
    let mut gamescope_args = use_signal(|| profile.gamescope_args.join(" "));
    let (w_init, h_init) = profile.resolution.unwrap_or((1920, 1080));
    let mut res_w = use_signal(|| w_init.to_string());
    let mut res_h = use_signal(|| h_init.to_string());
    let mut save_path = use_signal(|| {
        profile
            .save_data_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    });

    let mut gamemode = use_signal(|| profile.use_gamemode);
    let mut gamescope = use_signal(|| profile.use_gamescope);
    let mut mangohud = use_signal(|| profile.use_mangohud);
    let mut reshade = use_signal(|| profile.use_reshade);
    let mut proton_wayland = use_signal(|| profile.proton_wayland);
    let mut dxvk_low_latency = use_signal(|| profile.dxvk_low_latency);
    let mut obs_vkcapture = use_signal(|| profile.obs_vkcapture);
    let mut lsfg_vk = use_signal(|| profile.lsfg_vk);
    let mut enable_sfw = use_signal(|| profile.enable_sfw_toggle);
    let mut sfw_key = use_signal(|| {
        profile
            .sfw_toggle_key
            .clone()
            .unwrap_or_else(|| "F9".to_string())
    });
    let mut shuffle_launch = use_signal(|| profile.shuffle_on_launch);
    let mut context_swaps = use_signal(|| profile.enable_context_swaps);

    let pid = profile.id.to_string();

    // Mod list in load order
    let mut ordered_mods: Vec<(Uuid, bool, String)> = Vec::new();
    for mid in &profile.load_order {
        if let Some(m) = mods.get(mid) {
            let enabled = profile.enabled_mod_ids.contains(mid);
            ordered_mods.push((*mid, enabled, m.meta.name.clone()));
        }
    }
    for (mid, m) in &mods {
        if !profile.load_order.contains(mid) {
            let enabled = profile.enabled_mod_ids.contains(mid);
            ordered_mods.push((*mid, enabled, m.meta.name.clone()));
        }
    }

    let save_store = store.clone();
    let save_gid = game_id.clone();
    let save_pid = pid.clone();

    rsx! {
        div { class: "profile-detail",
            div { class: "profile-detail-head",
                div {
                    h2 { class: "profile-title", "{profile.name}" }
                    span { class: "setting-desc mono", "ID: {profile.id}" }
                }
                button {
                    class: "btn btn-accent",
                    onclick: move |_| {
                        let w = (res_w)().parse::<u32>().unwrap_or(1920);
                        let h = (res_h)().parse::<u32>().unwrap_or(1080);
                        let la: Vec<String> = (launch_args)().split_whitespace().map(|s| s.to_string()).collect();
                        let ga: Vec<String> = (gamescope_args)().split_whitespace().map(|s| s.to_string()).collect();
                        let sp = {
                            let p = (save_path)();
                            if p.trim().is_empty() { None } else { Some(std::path::PathBuf::from(p)) }
                        };

                        let update = ProfileUpdate {
                            description: Some((desc)()),
                            resolution: Some((w, h)),
                            launch_args: Some(la),
                            gamescope_args: Some(ga),
                            save_data_path: sp,
                            use_gamemode: Some((gamemode)()),
                            use_gamescope: Some((gamescope)()),
                            use_mangohud: Some((mangohud)()),
                            use_reshade: Some((reshade)()),
                            proton_wayland: Some((proton_wayland)()),
                            dxvk_low_latency: Some((dxvk_low_latency)()),
                            obs_vkcapture: Some((obs_vkcapture)()),
                            lsfg_vk: Some((lsfg_vk)()),
                            enable_sfw_toggle: Some((enable_sfw)()),
                            sfw_toggle_key: Some(Some((sfw_key)().trim().to_string())),
                            shuffle_on_launch: Some((shuffle_launch)()),
                            enable_context_swaps: Some((context_swaps)()),
                            ..Default::default()
                        };

                        let core = save_store.core.clone();
                        let gid = save_gid.clone();
                        let pid = save_pid.clone();
                        let s = save_store.clone();
                        s.call(
                            async move { core.update_profile(gid, pid, update).await },
                            |s, _| s.toast(ToastKind::Success, "Profile settings saved"),
                        );
                    },
                    IconSvg { name: IconName::Save, size: 14 }
                    "SAVE PROFILE"
                }
            }

            div { class: "settings-section",
                h3 { class: "settings-section-title mono", "PROFILE CONFIGURATION" }

                div { class: "field-group",
                    label { class: "field-label", "Description" }
                    input {
                        class: "input",
                        value: "{(desc)()}",
                        oninput: move |e| desc.set(e.value()),
                    }
                }

                div { class: "field-row",
                    div { class: "field-group half",
                        label { class: "field-label", "Virtual Resolution" }
                        div { class: "res-input-row",
                            input {
                                class: "input mono",
                                placeholder: "Width",
                                value: "{(res_w)()}",
                                oninput: move |e| res_w.set(e.value()),
                            }
                            span { class: "mono res-sep", "×" }
                            input {
                                class: "input mono",
                                placeholder: "Height",
                                value: "{(res_h)()}",
                                oninput: move |e| res_h.set(e.value()),
                            }
                        }
                    }
                    div { class: "field-group half",
                        label { class: "field-label", "Isolated Save Path" }
                        div { class: "path-editor-row",
                            input {
                                class: "input mono",
                                placeholder: "Default profile sandbox",
                                value: "{(save_path)()}",
                                oninput: move |e| save_path.set(e.value()),
                            }
                            button {
                                class: "btn",
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Some(p) = pick_folder("Select Save Directory").await {
                                            save_path.set(p.display().to_string());
                                        }
                                    });
                                },
                                IconSvg { name: IconName::Folder, size: 16 }
                            }
                        }
                    }
                }

                div { class: "field-row",
                    div { class: "field-group half",
                        label { class: "field-label", "Profile Launch Arguments" }
                        input {
                            class: "input mono",
                            placeholder: "-arg1 -arg2 ...",
                            value: "{(launch_args)()}",
                            oninput: move |e| launch_args.set(e.value()),
                        }
                    }
                    div { class: "field-group half",
                        label { class: "field-label", "Gamescope Arguments" }
                        input {
                            class: "input mono",
                            placeholder: "-W 1920 -H 1080 -r 144 ...",
                            value: "{(gamescope_args)()}",
                            oninput: move |e| gamescope_args.set(e.value()),
                        }
                    }
                }
            }

            div { class: "settings-section",
                h3 { class: "settings-section-title mono", "LINUX RUNTIME TOGGLES" }

                div { class: "toggle-grid",
                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "GameMode" }
                            div { class: "setting-desc", "CPU governor boost" }
                        }
                        button {
                            class: if (gamemode)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| gamemode.set(!(gamemode)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "Gamescope" }
                            div { class: "setting-desc", "Micro-compositor sandbox" }
                        }
                        button {
                            class: if (gamescope)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| gamescope.set(!(gamescope)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "MangoHud" }
                            div { class: "setting-desc", "In-game FPS overlay" }
                        }
                        button {
                            class: if (mangohud)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| mangohud.set(!(mangohud)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "ReShade" }
                            div { class: "setting-desc", "Post-process shaders" }
                        }
                        button {
                            class: if (reshade)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| reshade.set(!(reshade)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "Proton Wayland" }
                            div { class: "setting-desc", "Native Wayland driver" }
                        }
                        button {
                            class: if (proton_wayland)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| proton_wayland.set(!(proton_wayland)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "DXVK Low Latency" }
                            div { class: "setting-desc", "DXVK frame pacing boost" }
                        }
                        button {
                            class: if (dxvk_low_latency)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| dxvk_low_latency.set(!(dxvk_low_latency)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "OBS VkCapture" }
                            div { class: "setting-desc", "OBS Vulkan direct capture" }
                        }
                        button {
                            class: if (obs_vkcapture)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| obs_vkcapture.set(!(obs_vkcapture)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "LSFG-VK" }
                            div { class: "setting-desc", "Lossless Scaling Vulkan" }
                        }
                        button {
                            class: if (lsfg_vk)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| lsfg_vk.set(!(lsfg_vk)()),
                            div { class: "toggle-knob" }
                        }
                    }
                }
            }

            div { class: "settings-section",
                h3 { class: "settings-section-title mono", "WARDROBE & SMART MOD AUTOMATION" }

                div { class: "toggle-grid",
                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "In-Game SFW Switch" }
                            div { class: "setting-desc", "Single-key toggle hiding NSFW skins live in-game" }
                        }
                        button {
                            class: if (enable_sfw)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| enable_sfw.set(!(enable_sfw)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "Context-Aware Swaps" }
                            div { class: "setting-desc", "Land vs Swimming (Aqua) wardrobe pools" }
                        }
                        button {
                            class: if (context_swaps)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| context_swaps.set(!(context_swaps)()),
                            div { class: "toggle-knob" }
                        }
                    }

                    div { class: "toggle-card",
                        div {
                            div { class: "setting-label", "Shuffle Skins on Launch" }
                            div { class: "setting-desc", "Roll 1 random unlocked skin per character on start" }
                        }
                        button {
                            class: if (shuffle_launch)() { "toggle-switch on" } else { "toggle-switch" },
                            onclick: move |_| shuffle_launch.set(!(shuffle_launch)()),
                            div { class: "toggle-knob" }
                        }
                    }
                }

                if (enable_sfw)() {
                    div { class: "field-group", style: "margin-top: 10px; max-width: 240px;",
                        label { class: "field-label", "SFW Switch Keybind" }
                        input {
                            class: "input mono",
                            placeholder: "F9",
                            value: "{(sfw_key)()}",
                            oninput: move |e| sfw_key.set(e.value()),
                        }
                    }
                }
            }

            div { class: "settings-section",
                div { class: "profile-mods-head",
                    h3 { class: "settings-section-title mono", "LOADOUT MOD MATRIX" }
                    span { class: "mono setting-desc", "{profile.enabled_mod_ids.len()} / {ordered_mods.len()} ENABLED" }
                }

                if ordered_mods.is_empty() {
                    p { class: "wizard-note", "No mods installed for this game yet." }
                } else {
                    div { class: "profile-mods-list",
                        for (idx, (mid, enabled, mname)) in ordered_mods.iter().enumerate() {
                            {
                                let mid = *mid;
                                let is_en = *enabled;
                                let toggle_store = store.clone();
                                let toggle_gid = game_id.clone();
                                let toggle_pid = pid.clone();
                                let p_enabled = profile.enabled_mod_ids.clone();
                                let reorder_store = store.clone();
                                let reorder_gid = game_id.clone();
                                let reorder_pid = pid.clone();
                                rsx! {
                                    div {
                                        key: "{mid}",
                                        class: if is_en { "profile-mod-row enabled" } else { "profile-mod-row" },

                                        button {
                                            class: if is_en { "switch on" } else { "switch" },
                                            onclick: move |_| {
                                                let mut new_enabled = p_enabled.clone();
                                                if is_en {
                                                    new_enabled.retain(|id| *id != mid);
                                                } else {
                                                    new_enabled.push(mid);
                                                }
                                                let update = ProfileUpdate {
                                                    enabled_mod_ids: Some(new_enabled),
                                                    ..Default::default()
                                                };
                                                let core = toggle_store.core.clone();
                                                let gid = toggle_gid.clone();
                                                let pid = toggle_pid.clone();
                                                let s = toggle_store.clone();
                                                s.call(
                                                    async move { core.update_profile(gid, pid, update).await },
                                                    |_, _| {},
                                                );
                                            },
                                            span { class: "switch-knob" }
                                        }

                                        span { class: "profile-mod-name", "{mname}" }

                                        div { class: "profile-mod-reorder",
                                            button {
                                                class: "icon-btn",
                                                disabled: idx == 0,
                                                onclick: {
                                                    let gid = reorder_gid.clone();
                                                    let pid = reorder_pid.clone();
                                                    let s = reorder_store.clone();
                                                    move |_| {
                                                        do_reorder_profile_mod(s.clone(), gid.clone(), pid.clone(), mid, -1);
                                                    }
                                                },
                                                IconSvg { name: IconName::ChevronDown, size: 12 }
                                            }
                                            button {
                                                class: "icon-btn",
                                                disabled: idx == ordered_mods.len() - 1,
                                                onclick: {
                                                    let gid = reorder_gid.clone();
                                                    let pid = reorder_pid.clone();
                                                    let s = reorder_store.clone();
                                                    move |_| {
                                                        do_reorder_profile_mod(s.clone(), gid.clone(), pid.clone(), mid, 1);
                                                    }
                                                },
                                                IconSvg { name: IconName::ChevronRight, size: 12 }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn do_reorder_profile_mod(
    store: Store,
    game_id: String,
    profile_id: String,
    mod_id: Uuid,
    delta: i32,
) {
    let Some(db) = store.db(&game_id) else { return };
    let Ok(pid) = Uuid::parse_str(&profile_id) else {
        return;
    };
    let Some(profile) = db.profiles.get(&pid) else {
        return;
    };

    let mut order = profile.load_order.clone();
    for mid in db.mods.keys() {
        if !order.contains(mid) {
            order.push(*mid);
        }
    }
    let Some(idx) = order.iter().position(|id| *id == mod_id) else {
        return;
    };
    let new_idx = (idx as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
    if new_idx == idx {
        return;
    }
    order.swap(idx, new_idx);

    let update = ProfileUpdate {
        load_order: Some(order),
        ..Default::default()
    };
    let core = store.core.clone();
    let s = store.clone();
    s.call(
        async move { core.update_profile(game_id, profile_id, update).await },
        |_, _| {},
    );
}
