use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphicsAuditWarning {
    pub level: String, // "Warning", "Error", "Advisory"
    pub title: String,
    pub message: String,
    pub fixable: bool,
}

pub fn find_wuwa_engine_ini(game_dir: &Path) -> Option<PathBuf> {
    let candidate = game_dir
        .join("Client")
        .join("Saved")
        .join("Config")
        .join("WindowsNoEditor")
        .join("Engine.ini");
    if candidate.exists() {
        return Some(candidate);
    }

    // Also check Wuthering Waves Game subfolder
    let sub_candidate = game_dir
        .join("Wuthering Waves Game")
        .join("Client")
        .join("Saved")
        .join("Config")
        .join("WindowsNoEditor")
        .join("Engine.ini");
    if sub_candidate.exists() {
        return Some(sub_candidate);
    }

    Some(candidate)
}

pub fn audit_game_graphics(game_id: &str, game_dir: &Path) -> Vec<GraphicsAuditWarning> {
    let mut warnings = Vec::new();
    let gid = game_id.to_lowercase();

    // 1. Check for rogue d3dx.ini inside Mods/
    let rogue_mods_ini = game_dir.join("Mods").join("d3dx.ini");
    if rogue_mods_ini.exists() {
        warnings.push(GraphicsAuditWarning {
            level: "Error".to_string(),
            title: "Rogue d3dx.ini Inside Mods Folder".to_string(),
            message: "A d3dx.ini was found inside your Mods folder. 3DMigoto may override root settings with it, causing injection or keybind failures.".to_string(),
            fixable: false,
        });
    }

    // 2. Wuthering Waves Engine.ini LOD & Streaming Check
    if gid.contains("wuwa") || gid.contains("wuthering") {
        if let Some(engine_ini) = find_wuwa_engine_ini(game_dir) {
            let content = std::fs::read_to_string(&engine_ini).unwrap_or_default();
            let has_min_boost = content.contains("r.Streaming.MinBoost");
            let has_lod_fov = content.contains("DistanceLODBaseFOV");

            if !has_min_boost || !has_lod_fov {
                warnings.push(GraphicsAuditWarning {
                    level: "Warning".to_string(),
                    title: "Missing WuWa LOD & Texture Streaming Tweaks".to_string(),
                    message: "Engine.ini lacks r.Streaming.MinBoost or DistanceLODBaseFOV settings, which can cause mod textures to pop in late or render low-poly meshes.".to_string(),
                    fixable: true,
                });
            }
        }
    }

    // 3. Genshin / ZZZ Graphics Advisory
    if gid.contains("genshin") || gid.contains("zzz") || gid.contains("zenless") {
        warnings.push(GraphicsAuditWarning {
            level: "Advisory".to_string(),
            title: "Graphics Settings Recommendation".to_string(),
            message: "Ensure in-game Graphics Settings have 'Dynamic Character Resolution' set to OFF and 'Character Quality' set to High for optimal shader and texture mod accuracy.".to_string(),
            fixable: false,
        });
    }

    warnings
}

pub fn apply_wuwa_engine_tweaks(game_dir: &Path) -> std::io::Result<()> {
    let engine_ini = find_wuwa_engine_ini(game_dir).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine Engine.ini path",
        )
    })?;

    if let Some(parent) = engine_ini.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = std::fs::read_to_string(&engine_ini).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();

    let tweaks = [
        "r.Streaming.MinBoost=1.0",
        "r.Streaming.PoolSize=0",
        "r.Streaming.LimitPoolSizeToVRAM=0",
        "r.DistanceLODBaseFOV=90.0",
    ];

    let has_system_settings = lines
        .iter()
        .any(|l| l.trim().eq_ignore_ascii_case("[SystemSettings]"));

    if has_system_settings {
        let pos = lines
            .iter()
            .position(|l| l.trim().eq_ignore_ascii_case("[SystemSettings]"))
            .unwrap();

        let mut insert_pos = pos + 1;
        for tweak in tweaks {
            let key = tweak.split('=').next().unwrap();
            let already_has = lines.iter().any(|l| l.trim().starts_with(key));
            if !already_has {
                lines.insert(insert_pos, tweak.to_string());
                insert_pos += 1;
            }
        }
    } else {
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push("[SystemSettings]".to_string());
        for tweak in tweaks {
            lines.push(tweak.to_string());
        }
    }

    lines.push(String::new());
    std::fs::write(&engine_ini, lines.join("\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wuwa_audit_and_apply_tweaks() {
        let dir = tempdir().unwrap();
        let game_dir = dir.path();

        // 1. Initial audit on fresh WuWa directory: missing tweaks warning
        let warnings = audit_game_graphics("wuwa", game_dir);
        assert!(
            warnings
                .iter()
                .any(|w| w.fixable && w.title.contains("Missing WuWa")),
            "Expected missing tweaks warning"
        );

        // 2. Apply tweaks
        apply_wuwa_engine_tweaks(game_dir).unwrap();

        // 3. Re-audit: warning must now be gone
        let warnings_after = audit_game_graphics("wuwa", game_dir);
        assert!(
            !warnings_after
                .iter()
                .any(|w| w.title.contains("Missing WuWa")),
            "Warning should be resolved after applying tweaks"
        );

        // 4. Check Engine.ini content
        let ini_path = find_wuwa_engine_ini(game_dir).unwrap();
        let content = std::fs::read_to_string(ini_path).unwrap();
        assert!(content.contains("[SystemSettings]"));
        assert!(content.contains("r.Streaming.MinBoost=1.0"));
        assert!(content.contains("r.DistanceLODBaseFOV=90.0"));
    }

    #[test]
    fn test_rogue_d3dx_ini_audit() {
        let dir = tempdir().unwrap();
        let game_dir = dir.path();
        let mods_dir = game_dir.join("Mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::write(mods_dir.join("d3dx.ini"), "; rogue").unwrap();

        let warnings = audit_game_graphics("genshin", game_dir);
        assert!(
            warnings
                .iter()
                .any(|w| w.level == "Error" && w.title.contains("Rogue d3dx.ini")),
            "Expected rogue d3dx.ini error"
        );
    }
}
