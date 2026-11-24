use resources::registry::{RegistrySource, RegistrySyncReport};
use std::collections::HashMap;
use storage::GameTemplate;

use crate::events::AppEvent;
use crate::state::AppCore;

fn slugify(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        format!("reg-{}", uuid::Uuid::new_v4().simple())
    } else {
        trimmed.to_string()
    }
}

impl AppCore {
    /// Returns the list of all configured registry sources.
    pub async fn list_registries(&self) -> Result<Vec<RegistrySource>, String> {
        let config = self.app_config.lock().await;
        Ok(config.registries.clone())
    }

    /// Adds a new third-party registry source and saves the configuration.
    pub async fn add_registry(&self, name: String, url: String) -> Result<RegistrySource, String> {
        let trimmed_name = name.trim();
        let trimmed_url = url.trim();

        if trimmed_name.is_empty() {
            return Err("Registry name cannot be empty".to_string());
        }
        if trimmed_url.is_empty() {
            return Err("Registry URL cannot be empty".to_string());
        }

        let mut config = self.app_config.lock().await.clone();

        let base_slug = slugify(trimmed_name);
        let mut id = base_slug.clone();
        let mut counter = 1;
        while config.registries.iter().any(|r| r.id == id) {
            id = format!("{}-{}", base_slug, counter);
            counter += 1;
        }

        let new_source = RegistrySource {
            id: id.clone(),
            name: trimmed_name.to_string(),
            url: trimmed_url.to_string(),
            enabled: true,
            official: false,
        };

        config.registries.push(new_source.clone());

        let config_path = self.app_data_dir.join("app_config.json");
        storage::write_json_atomic(&config_path, &config).map_err(|e| e.to_string())?;

        *self.app_config.lock().await = config.clone();
        self.emit(AppEvent::AppConfigUpdated(config));

        Ok(new_source)
    }

    /// Removes a custom registry source and deletes its local cache directory.
    pub async fn remove_registry(&self, registry_id: String) -> Result<(), String> {
        let mut config = self.app_config.lock().await.clone();

        if let Some(pos) = config.registries.iter().position(|r| r.id == registry_id) {
            if config.registries[pos].official {
                return Err("Cannot remove the official YAGO registry".to_string());
            }
            config.registries.remove(pos);

            let reg_dir = self.app_data_dir.join("registry").join(&registry_id);
            if reg_dir.exists() {
                let _ = std::fs::remove_dir_all(&reg_dir);
            }

            let config_path = self.app_data_dir.join("app_config.json");
            storage::write_json_atomic(&config_path, &config).map_err(|e| e.to_string())?;

            *self.app_config.lock().await = config.clone();
            self.emit(AppEvent::AppConfigUpdated(config));
            Ok(())
        } else {
            Err(format!("Registry '{}' not found", registry_id))
        }
    }

    /// Toggles whether a registry source is enabled.
    pub async fn toggle_registry(&self, registry_id: String, enabled: bool) -> Result<(), String> {
        let mut config = self.app_config.lock().await.clone();

        if let Some(reg) = config.registries.iter_mut().find(|r| r.id == registry_id) {
            reg.enabled = enabled;

            let config_path = self.app_data_dir.join("app_config.json");
            storage::write_json_atomic(&config_path, &config).map_err(|e| e.to_string())?;

            *self.app_config.lock().await = config.clone();
            self.emit(AppEvent::AppConfigUpdated(config));
            Ok(())
        } else {
            Err(format!("Registry '{}' not found", registry_id))
        }
    }

    /// Loads all game templates: bundled templates_root first, then overlays any games/*.json
    /// from enabled synced registries (app_data_dir/registry/<id>/games/*.json).
    pub async fn load_all_templates(&self) -> HashMap<String, GameTemplate> {
        let templates_root = self.librarian.lock().await.templates_root.clone();
        let registry = storage::TemplateRegistry::new(templates_root);
        let mut templates = registry.load_all().await.unwrap_or_default();

        let enabled_registries: Vec<String> = {
            let config = self.app_config.lock().await;
            config
                .registries
                .iter()
                .filter(|r| r.enabled)
                .map(|r| r.id.clone())
                .collect()
        };

        for reg_id in enabled_registries {
            let reg_games_dir = self
                .app_data_dir
                .join("registry")
                .join(&reg_id)
                .join("games");
            if reg_games_dir.exists() {
                if let Ok(reg_templates) = storage::template::load_templates(&reg_games_dir) {
                    for template in reg_templates {
                        if !template.executables.is_empty() {
                            for exe in &template.executables {
                                templates.insert(exe.to_lowercase(), template.clone());
                            }
                        }
                        templates.insert(template.id.to_lowercase(), template);
                    }
                }
            }
        }

        templates
    }

    /// Syncs all enabled registries into app_data_dir/registry/, emitting event notifications.
    pub async fn sync_all_registries(&self) -> Result<Vec<RegistrySyncReport>, String> {
        let registries: Vec<RegistrySource> = {
            let config = self.app_config.lock().await;
            config
                .registries
                .iter()
                .filter(|r| r.enabled)
                .cloned()
                .collect()
        };

        let cache_root = self.app_data_dir.join("registry");
        let _ = std::fs::create_dir_all(&cache_root);

        let current_version = env!("CARGO_PKG_VERSION");
        let mut reports = Vec::new();
        let mut warnings = Vec::new();

        for source in registries {
            match resources::sync_registry(&source, &cache_root, current_version).await {
                Ok(report) => {
                    if !report.failed.is_empty() {
                        warnings.push(format!(
                            "Registry '{}' had {} failed file(s)",
                            source.name,
                            report.failed.len()
                        ));
                    }
                    reports.push(report);
                }
                Err(e) => {
                    tracing::warn!("Failed to sync registry '{}': {}", source.name, e);
                    warnings.push(format!("Registry '{}': {}", source.name, e));
                }
            }
        }

        // Overlay synced hashes into librarian assets_root/hashes/
        let (assets_root, enabled_reg_ids) = {
            let librarian = self.librarian.lock().await;
            let config = self.app_config.lock().await;
            let reg_ids: Vec<String> = config
                .registries
                .iter()
                .filter(|r| r.enabled)
                .map(|r| r.id.clone())
                .collect();
            (librarian.assets_root.clone(), reg_ids)
        };

        let target_hashes_dir = assets_root.join("hashes");
        let _ = std::fs::create_dir_all(&target_hashes_dir);

        for reg_id in &enabled_reg_ids {
            let reg_hashes_dir = self
                .app_data_dir
                .join("registry")
                .join(reg_id)
                .join("hashes");
            if reg_hashes_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&reg_hashes_dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("json")
                        {
                            let dest = target_hashes_dir.join(entry.file_name());
                            let _ = std::fs::copy(&path, &dest);
                            if entry.file_name() == "hashes.json" {
                                let _ = std::fs::copy(&path, assets_root.join("hashes.json"));
                            }
                        }
                    }
                }
            }
        }

        // Reload templates with synced registry overlays
        let new_templates = self.load_all_templates().await;
        *self.game_templates.lock().await = new_templates;
        self.emit_library().await;

        let total_downloaded: usize = reports.iter().map(|r| r.downloaded).sum();
        let total_skipped: usize = reports.iter().map(|r| r.skipped_unchanged).sum();

        if !warnings.is_empty() {
            self.emit(AppEvent::Warning(warnings.join(" • ")));
        } else {
            self.emit(AppEvent::TaskCompleted(format!(
                "Registry sync completed ({} downloaded, {} up-to-date)",
                total_downloaded, total_skipped
            )));
        }

        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_registry_management_lifecycle() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        // 1. Initial list has official registry
        let list = core.list_registries().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "official");
        assert!(list[0].official);

        // 2. Add custom registry
        let added = core
            .add_registry(
                "Community Test".to_string(),
                "https://example.com/reg".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(added.id, "community-test");
        assert!(!added.official);
        assert!(added.enabled);

        let list_after_add = core.list_registries().await.unwrap();
        assert_eq!(list_after_add.len(), 2);

        // 3. Toggle custom registry
        core.toggle_registry("community-test".to_string(), false)
            .await
            .unwrap();
        let list_after_toggle = core.list_registries().await.unwrap();
        let comm = list_after_toggle
            .iter()
            .find(|r| r.id == "community-test")
            .unwrap();
        assert!(!comm.enabled);

        // 4. Removing official registry is rejected
        let remove_official_res = core.remove_registry("official".to_string()).await;
        assert!(remove_official_res.is_err());

        // 5. Remove custom registry
        core.remove_registry("community-test".to_string())
            .await
            .unwrap();
        let list_after_remove = core.list_registries().await.unwrap();
        assert_eq!(list_after_remove.len(), 1);
        assert_eq!(list_after_remove[0].id, "official");
    }

    #[tokio::test]
    async fn test_registry_overlay_overrides_template_and_fixer() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();
        core.sync_templates().await.unwrap();

        // 1. Check bundled template for genshin exists
        {
            let templates = core.game_templates.lock().await;
            let genshin = templates.get("genshin").expect("Genshin template missing");
            assert_ne!(genshin.description, "Overridden by registry");
        }

        // 2. Prepare mock registry
        let reg_temp = tempdir().unwrap();
        let games_dir = reg_temp.path().join("games");
        let fixers_dir = reg_temp.path().join("fixers");
        std::fs::create_dir_all(&games_dir).unwrap();
        std::fs::create_dir_all(&fixers_dir).unwrap();

        // Mock games/genshin.json
        let modified_template = storage::GameTemplate {
            id: "genshin".to_string(),
            name: "Genshin Impact".to_string(),
            description: "Overridden by registry".to_string(),
            gamebanana_id: Some(99999),
            loader_id: Some("xxmi-3dmigoto".to_string()),
            executables: vec!["GenshinImpact.exe".to_string()],
            ..Default::default()
        };
        let template_bytes = serde_json::to_vec_pretty(&modified_template).unwrap();
        std::fs::write(games_dir.join("genshin.json"), &template_bytes).unwrap();

        // Mock fixers/genshin.json
        let mut custom_fixer = mod_patches::FixRuleSet {
            game_id: "genshin".to_string(),
            description: "Overridden registry fixer".to_string(),
            apply_orfix: true,
            ..Default::default()
        };
        custom_fixer
            .hash_migrations
            .insert("old_hash".to_string(), "new_hash".to_string());
        let fixer_bytes = serde_json::to_vec_pretty(&custom_fixer).unwrap();
        std::fs::write(fixers_dir.join("genshin.json"), &fixer_bytes).unwrap();

        // Build index.json
        let index = resources::RegistryIndex {
            schema_version: 1,
            registry_version: "1.0.0".to_string(),
            min_yago_version: Some("0.1.0".to_string()),
            updated_at: "2026-11-24T00:00:00Z".to_string(),
            entries: vec![
                resources::RegistryEntry {
                    path: "games/genshin.json".to_string(),
                    category: "game".to_string(),
                    sha256: resources::compute_sha256(&template_bytes),
                    size_bytes: Some(template_bytes.len() as u64),
                },
                resources::RegistryEntry {
                    path: "fixers/genshin.json".to_string(),
                    category: "fixer".to_string(),
                    sha256: resources::compute_sha256(&fixer_bytes),
                    size_bytes: Some(fixer_bytes.len() as u64),
                },
            ],
        };
        let index_bytes = serde_json::to_vec_pretty(&index).unwrap();
        std::fs::write(reg_temp.path().join("index.json"), index_bytes).unwrap();

        // 3. Register and sync custom registry
        let reg_url = format!("file://{}", reg_temp.path().display());
        core.add_registry("Mock Overlay".to_string(), reg_url)
            .await
            .unwrap();

        let sync_reports = core.sync_all_registries().await.unwrap();
        let overlay_report = sync_reports
            .iter()
            .find(|r| r.registry_id == "mock-overlay")
            .unwrap();
        assert_eq!(overlay_report.downloaded, 2);

        // 4. Verify game template is overridden in AppCore
        {
            let templates = core.game_templates.lock().await;
            let genshin = templates.get("genshin").unwrap();
            assert_eq!(genshin.description, "Overridden by registry");
            assert_eq!(genshin.gamebanana_id, Some(99999));
        }

        // 5. Verify fixer rules are overridden in AppCore
        {
            let librarian = core.librarian.lock().await;
            let rules = core.load_fix_rules(&librarian.assets_root, "genshin");
            assert_eq!(rules.description, "Overridden registry fixer");
            assert_eq!(
                rules.hash_migrations.get("old_hash"),
                Some(&"new_hash".to_string())
            );
        }
    }
}
