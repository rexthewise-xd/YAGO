#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use storage::GameTemplate;
    use tempfile::tempdir;

    use crate::events::AppEvent;
    use crate::init::init_with_dir;

    #[test]
    fn test_fuzzy_template_matching() {
        let mut templates = HashMap::new();
        templates.insert(
            "genshin".to_string(),
            GameTemplate {
                id: "genshin".into(),
                name: "Genshin Impact".into(),
                short_name: "Genshin".into(),
                ..Default::default()
            },
        );
        templates.insert(
            "hsr".to_string(),
            GameTemplate {
                id: "hsr".into(),
                name: "Honkai: Star Rail".into(),
                short_name: "HSR".into(),
                ..Default::default()
            },
        );

        let normalize = |s: &str| -> String {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect()
        };

        let find_fuzzy = |id: &str, temps: &HashMap<String, GameTemplate>| -> Option<String> {
            temps
                .get(id)
                .or_else(|| {
                    let base = id.trim_end_matches(".exe");
                    temps.get(base).or_else(|| {
                        let base_norm = normalize(base);
                        temps.values().find(|t| {
                            let name_norm = normalize(&t.name);
                            let short_norm = normalize(&t.short_name);
                            base_norm.contains(&short_norm)
                                || base_norm.contains(&name_norm)
                                || short_norm.contains(&base_norm)
                                || name_norm.contains(&base_norm)
                        })
                    })
                })
                .map(|t| t.id.clone())
        };

        assert_eq!(find_fuzzy("genshin", &templates).unwrap(), "genshin");
        assert_eq!(find_fuzzy("genshin.exe", &templates).unwrap(), "genshin");
        assert_eq!(find_fuzzy("starrail.exe", &templates).unwrap(), "hsr");
        assert_eq!(
            find_fuzzy("GenshinImpact.exe", &templates).unwrap(),
            "genshin"
        );
    }

    #[tokio::test]
    async fn test_init_extracts_bundled_assets() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        assert!(dir.path().join("app_config.json").exists());
        let librarian = core.librarian.lock().await;
        assert!(librarian.assets_root.join("hashes.json").exists());
        assert!(librarian.templates_root.join("genshin.json").exists());
        assert!(librarian
            .assets_root
            .join("fixers")
            .join("genshin.json")
            .exists());

        let config_content = fs::read_to_string(dir.path().join("app_config.json")).unwrap();
        assert!(config_content.contains("proton_repo"));

        #[cfg(target_os = "linux")]
        assert!(dir.path().join("libs").join("libshield.so").exists());
    }

    #[tokio::test]
    async fn test_settings_roundtrip() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let mut settings = core.get_settings().await.unwrap();
        settings.language = "ja-JP".to_string();
        settings.stream_safe = false;
        core.update_settings(settings).await.unwrap();

        let loaded = core.get_settings().await.unwrap();
        assert_eq!(loaded.language, "ja-JP");
        assert!(!loaded.stream_safe);

        let on_disk: storage::GlobalSettings =
            serde_json::from_str(&fs::read_to_string(dir.path().join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(on_disk.language, "ja-JP");
    }

    #[tokio::test]
    async fn test_app_config_roundtrip() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let mut config = core.get_app_config().await.unwrap();
        config.theme = "nord".to_string();
        core.update_app_config(config).await.unwrap();

        assert_eq!(core.get_app_config().await.unwrap().theme, "nord");
    }

    #[tokio::test]
    async fn test_event_bus() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();
        let mut rx = core.events.subscribe();

        core.emit(AppEvent::TaskCompleted("test".to_string()));
        match rx.recv().await.unwrap() {
            AppEvent::TaskCompleted(msg) => assert_eq!(msg, "test"),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_runner_resolution_logic() {
        use std::path::PathBuf;
        let dir = tempdir().unwrap();
        let app_data = dir.path().join("app_data");
        let runners_dir = app_data.join("runners/GE-Proton-1");
        std::fs::create_dir_all(&runners_dir).unwrap();
        std::fs::File::create(runners_dir.join("proton")).unwrap();

        let settings = storage::GlobalSettings {
            steam_compat_tools_path: PathBuf::from("/tmp/steam_mock"),
            ..Default::default()
        };

        let (path, rtype) = crate::commands::launcher::resolve_runner_path(
            Some("GE-Proton-1".to_string()),
            &app_data,
            &settings,
        )
        .await;
        assert_eq!(rtype, launcher::RunnerType::Proton);
        assert!(path
            .to_string_lossy()
            .contains("app_data/runners/GE-Proton-1/proton"));

        let (path, rtype) =
            crate::commands::launcher::resolve_runner_path(None, &app_data, &settings).await;
        assert_eq!(rtype, launcher::RunnerType::Wine);
        assert_eq!(path.to_string_lossy(), "wine");
    }

    #[tokio::test]
    async fn test_remote_game_lifecycle() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        core.sync_templates().await.unwrap();
        let templates = core.game_templates.lock().await;
        let some_id = templates.keys().next().cloned();
        drop(templates);
        let game_id = some_id.expect("bundled templates should load");

        let id = core.initialize_remote_game(game_id.clone()).await.unwrap();
        assert_eq!(id, game_id);
        assert!(core.get_library().await.unwrap().contains_key(&game_id));

        let profile = core
            .create_profile(game_id.clone(), "Test Loadout".to_string())
            .await
            .unwrap();
        assert_eq!(profile.name, "Test Loadout");
        core.switch_profile(game_id.clone(), profile.id.to_string())
            .await
            .unwrap();

        core.remove_game(game_id.clone()).await.unwrap();
        assert!(!core.get_library().await.unwrap().contains_key(&game_id));
    }

    #[tokio::test]
    async fn test_fresh_device_setup_lifecycle() {
        let dir = tempdir().unwrap();
        let app_data_dir = dir.path().to_path_buf();

        // 1. Boot init_with_dir on a completely empty directory
        let core = init_with_dir(app_data_dir.clone()).await.unwrap();

        // 2. Assert core.check_setup().await.unwrap() == false
        assert_eq!(core.check_setup().await.unwrap(), false);

        // 3. Call core.update_settings(GlobalSettings::default()).await.unwrap()
        core.update_settings(storage::GlobalSettings::default())
            .await
            .unwrap();

        // 4. Assert core.check_setup().await.unwrap() == true and settings.json exists inside data dir
        assert_eq!(core.check_setup().await.unwrap(), true);
        assert!(app_data_dir.join("settings.json").exists());
    }
}
