use std::path::PathBuf;

use storage::models::{GameConfig, LibraryDatabase, ModRecord, Profile};
use uuid::Uuid;

use crate::art::{encode_asset_url, FALLBACK_COVER};
use crate::state::Store;

pub const NSFW_TAGS: [&str; 7] = [
    "nsfw", "nude", "bikini", "skimpy", "hentai", "18+", "explicit",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveDir {
    Up,
    Down,
    Top,
    Bottom,
}

pub fn is_nsfw(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| NSFW_TAGS.contains(&t.to_lowercase().as_str()))
}

pub fn tag_class(tag: &str) -> &'static str {
    let lower = tag.to_lowercase();
    if NSFW_TAGS.contains(&lower.as_str()) {
        "tag tag-danger"
    } else if ["ui", "hud", "interface", "menu"].contains(&lower.as_str()) {
        "tag tag-blue"
    } else if ["texture", "environment", "map", "world", "landscape"].contains(&lower.as_str()) {
        "tag tag-accent"
    } else if ["audio", "sound", "music", "voice"].contains(&lower.as_str()) {
        "tag tag-amber"
    } else if ["shader", "reshade", "visuals", "graphics"].contains(&lower.as_str()) {
        "tag tag-purple"
    } else if ["character", "outfit", "skin", "model"].contains(&lower.as_str()) {
        "tag tag-pink"
    } else if ["scripts", "tools", "cheat", "utility"].contains(&lower.as_str()) {
        "tag tag-cyan"
    } else {
        "tag"
    }
}

#[allow(dead_code)]
pub fn parse_size(size_str: &str) -> u64 {
    let trimmed = size_str.trim();
    let num_end = trimmed
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(trimmed.len());
    let Ok(val) = trimmed[..num_end].trim().parse::<f64>() else {
        return 0;
    };
    let unit = trimmed[num_end..].trim().to_uppercase();
    let mult = match unit.as_str() {
        "GB" => 1024 * 1024 * 1024,
        "MB" => 1024 * 1024,
        "KB" => 1024,
        _ => 1,
    };
    (val * mult as f64) as u64
}

pub fn active_profile<'a>(db: &'a LibraryDatabase, game: &GameConfig) -> Option<&'a Profile> {
    Uuid::parse_str(&game.active_profile_id)
        .ok()
        .and_then(|id| db.profiles.get(&id))
        .or_else(|| db.profiles.values().next())
}

#[allow(dead_code)]
pub fn mod_enabled(db: &LibraryDatabase, game: &GameConfig, mod_id: Uuid) -> bool {
    active_profile(db, game)
        .map(|p| p.enabled_mod_ids.contains(&mod_id))
        .unwrap_or(false)
}

/// Resolve a mod's preview image to a loadable URL. Preview references are
/// relative to the mod directory, absolute paths, or pre-encoded asset URLs.
pub fn mod_preview_url(_store: &Store, record: &ModRecord) -> Option<String> {
    let raw = record.meta.preview_image.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("yago-asset://") || raw.starts_with("http://") || raw.starts_with("https://")
    {
        return Some(raw.to_string());
    }
    let path = PathBuf::from(raw);
    let resolved = if path.is_absolute() {
        path
    } else {
        record.path.join(raw)
    };
    if resolved.exists() {
        Some(encode_asset_url(&resolved))
    } else {
        None
    }
}

pub fn mod_preview_or_fallback(store: &Store, record: &ModRecord) -> String {
    mod_preview_url(store, record).unwrap_or_else(|| FALLBACK_COVER.to_string())
}