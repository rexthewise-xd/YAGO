use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use crate::state::Store;

pub static FALLBACK_COVER: Asset = asset!("/assets/genshin_background.webp");

fn encode_url(path: &Path) -> String {
    format!(
        "yago-asset://localhost/{}",
        urlencoding::encode(&path.to_string_lossy())
    )
}

pub fn encode_asset_url(path: &Path) -> String {
    encode_url(path)
}

fn decode_asset(raw: &str) -> Option<PathBuf> {
    let rest = raw.strip_prefix("yago-asset://")?;
    let path_part = rest.split_once('/')?.1;
    let decoded = urlencoding::decode(path_part).ok()?;
    Some(PathBuf::from(decoded.into_owned()))
}

/// Resolve a stored cover/icon reference to a loadable URL.
/// Handles absolute paths, bare template filenames, and stale
/// `yago-asset://` URLs pointing at another machine's home dir.
pub fn art_url(store: &Store, raw: &str) -> String {
    if raw.is_empty() {
        return FALLBACK_COVER.to_string();
    }

    let templates_dir = store.core.app_data_dir.join("templates");

    if raw.starts_with("yago-asset://") {
        if let Some(path) = decode_asset(raw) {
            if path.exists() {
                return raw.to_string();
            }
            if let Some(name) = path.file_name() {
                let local = templates_dir.join(name);
                if local.exists() {
                    return encode_url(&local);
                }
            }
        }
        return FALLBACK_COVER.to_string();
    }

    let path = PathBuf::from(raw);
    if path.is_absolute() {
        if path.exists() {
            return encode_url(&path);
        }
        return FALLBACK_COVER.to_string();
    }

    let local = templates_dir.join(raw);
    if local.exists() {
        return encode_url(&local);
    }

    FALLBACK_COVER.to_string()
}

pub fn cover_url(store: &Store, game: &storage::GameConfig) -> String {
    art_url(store, &game.cover_image)
}

pub fn icon_url(store: &Store, game: &storage::GameConfig) -> String {
    art_url(store, &game.icon)
}