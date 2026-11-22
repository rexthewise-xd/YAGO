use crate::error::Result;
use serde_json::Value;

pub struct CloudSync;

impl CloudSync {
    /// Fetches latest game hashes from a remote repository.
    /// URL can be a full HTTP/HTTPS URL or a GitHub "owner/repo" slug.
    pub async fn fetch_latest_hashes(url: &str) -> Result<Value> {
        let full_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://raw.githubusercontent.com/{}/main/hashes.json", url)
        };
        let client = reqwest::Client::new();
        let response = client
            .get(&full_url)
            .header("User-Agent", "YAGO-Librarian/1.0")
            .send()
            .await?;

        let hashes = response.json::<Value>().await?;
        Ok(hashes)
    }

    /// Stub for future game DB cloud sync.
    pub async fn sync_game_db(
        &self,
        _game_id: &str,
        _db: &crate::models::LibraryDatabase,
    ) -> Result<()> {
        Ok(())
    }
}
