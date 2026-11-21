use crate::state::AppCore;

impl AppCore {
    pub async fn resolve_asset(&self, url: String) -> Result<String, String> {
        let cache_dir = {
            let settings = self.global_settings.lock().await;
            if !settings.cache_path.as_os_str().is_empty() {
                settings.cache_path.join("assets")
            } else {
                self.app_data_dir.join("cache").join("assets")
            }
        };
        let cache = resources::AssetCache::new(cache_dir);

        match cache.resolve(&url).await {
            Ok(path) => {
                let path_str = path.to_string_lossy().to_string();
                let encoded = urlencoding::encode(&path_str);
                Ok(format!("yago-asset://{}", encoded))
            }
            Err(e) => Err(format!("Failed to resolve asset: {}", e)),
        }
    }

    pub async fn get_community_backgrounds(&self, game_id: String) -> Result<Vec<String>, String> {
        let (repo, base_url) = {
            let config = self.app_config.lock().await;
            (
                config.community_backgrounds_repo.clone(),
                config.community_backgrounds_base_url.clone(),
            )
        };

        let folder = {
            let templates = self.game_templates.lock().await;
            templates
                .get(&game_id)
                .and_then(|t| t.community_folder.clone())
                .ok_or_else(|| format!("No community folder defined for game: {}", game_id))?
        };

        let client = reqwest::Client::builder()
            .user_agent("YAGO-Organizer")
            .build()
            .map_err(|e| e.to_string())?;

        let tree_url = format!(
            "https://api.github.com/repos/{}/git/trees/main?recursive=1",
            repo
        );

        let response = client
            .get(&tree_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("Failed to fetch repo tree: {}", response.status()));
        }

        let contents: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        let mut urls = Vec::new();

        if let Some(tree) = contents.get("tree").and_then(|t| t.as_array()) {
            let search_token = format!("/{}/", folder);

            for entry in tree {
                if let (Some(path), Some(entry_type)) = (
                    entry.get("path").and_then(|p| p.as_str()),
                    entry.get("type").and_then(|t| t.as_str()),
                ) {
                    if entry_type == "blob"
                        && path.starts_with("output/")
                        && path.contains(&search_token)
                        && path.contains("_pure")
                        && (path.ends_with(".png")
                            || path.ends_with(".webp")
                            || path.ends_with(".jpg")
                            || path.ends_with(".jpeg"))
                    {
                        let relative_path = &path[7..];
                        urls.push(format!("{}/{}", base_url, relative_path));
                    }
                }
            }
        }

        Ok(urls)
    }
}