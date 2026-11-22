use anyhow::{anyhow, Result};
use ini::ast::IniItem;
use ini::IniCompiler;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct FixRuleSet {
    pub game_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub hash_migrations: HashMap<String, String>, // old hash -> new hash
    #[serde(default)]
    pub slot_remaps: Vec<SlotRemapRule>,
    #[serde(default)]
    pub apply_orfix: bool,
    #[serde(default)]
    pub face_fix_ps_t0_to_this: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct SlotRemapRule {
    #[serde(default)]
    pub section_contains: String, // case-insensitive substring filter, empty = all TextureOverride*
    pub old_key: String,
    pub new_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FileFixDiff {
    pub relative_path: String,
    pub diff: String,
    pub changes_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct FixPreview {
    pub file_diffs: Vec<FileFixDiff>,
    pub total_hash_fixes: usize,
    pub total_slot_fixes: usize,
    pub total_orfix_fixes: usize,
    pub skipped_noapplier: Vec<String>,
}

impl FixPreview {
    pub fn has_changes(&self) -> bool {
        self.total_hash_fixes > 0 || self.total_slot_fixes > 0 || self.total_orfix_fixes > 0
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct FixApplyReport {
    pub backup_path: String,
    pub files_modified: usize,
    pub total_hash_fixes: usize,
    pub total_slot_fixes: usize,
    pub total_orfix_fixes: usize,
}

/// Generates a unified-style diff showing lines changed.
pub fn generate_unified_diff(old_text: &str, new_text: &str) -> String {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let mut out = String::new();
    let mut i = 0;
    let mut j = 0;

    while i < old_lines.len() || j < new_lines.len() {
        if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
            i += 1;
            j += 1;
        } else {
            if i < old_lines.len() {
                out.push_str(&format!("- {}\n", old_lines[i]));
                i += 1;
            }
            if j < new_lines.len() {
                out.push_str(&format!("+ {}\n", new_lines[j]));
                j += 1;
            }
        }
    }

    out
}

struct TransformResult {
    new_content: String,
    hash_fixes: usize,
    slot_fixes: usize,
    orfix_fixes: usize,
}

fn transform_ini_content(content: &str, rules: &FixRuleSet) -> Result<Option<TransformResult>> {
    let (_, mut doc) =
        ini::parser::parse_ini(content).map_err(|e| anyhow!("Failed to parse INI: {:?}", e))?;

    let mut hash_fixes = 0usize;
    let mut slot_fixes = 0usize;
    let mut orfix_fixes = 0usize;

    for section in &mut doc.sections {
        let sec_name_lower = section.name.to_lowercase();
        let is_texture_override = sec_name_lower.starts_with("textureoverride");

        // 1. Hash migrations (any section)
        for item in &mut section.items {
            if let IniItem::Pair { key, value } = item {
                if key.trim().eq_ignore_ascii_case("hash") {
                    let val_lower = value.trim().to_lowercase();
                    for (old_h, new_h) in &rules.hash_migrations {
                        if old_h.trim().eq_ignore_ascii_case(&val_lower)
                            && value.trim() != new_h.trim()
                        {
                            *value = new_h.clone();
                            hash_fixes += 1;
                            break;
                        }
                    }
                }
            }
        }

        // 2. Slot remaps (in matching TextureOverride sections)
        if is_texture_override {
            for remap in &rules.slot_remaps {
                let match_section = remap.section_contains.is_empty()
                    || sec_name_lower.contains(&remap.section_contains.to_lowercase());

                if match_section {
                    for item in &mut section.items {
                        if let IniItem::Pair { key, .. } = item {
                            if key.trim().eq_ignore_ascii_case(&remap.old_key) {
                                *key = remap.new_key.clone();
                                slot_fixes += 1;
                            }
                        }
                    }
                }
            }
        }

        // 3. ORFix & FaceFix
        if rules.apply_orfix && is_texture_override {
            let is_face = sec_name_lower.contains("faceheaddiffuse");

            if is_face {
                // Prevent white cheek artifacts: FaceHeadDiffuse is excluded from ORFix
                if rules.face_fix_ps_t0_to_this {
                    for item in &mut section.items {
                        if let IniItem::Pair { key, value } = item {
                            if key.trim().eq_ignore_ascii_case("ps-t0") && value.trim() != "this" {
                                *value = "this".to_string();
                                slot_fixes += 1;
                            }
                        }
                    }
                }
            } else {
                // Non-face section: check if NormalMap or (ps-t0 + ps-t1 + ps-t2)
                let mut has_ps_t0 = false;
                let mut has_ps_t1 = false;
                let mut has_ps_t2 = false;
                let mut has_normal_map = false;
                let mut has_orfix_call = false;
                let mut texfx_transparency_pos = None;

                for (idx, item) in section.items.iter().enumerate() {
                    match item {
                        IniItem::Pair { key, value } => {
                            let k = key.trim().to_lowercase();
                            let v = value.trim().to_lowercase();

                            if k == "ps-t0" {
                                has_ps_t0 = true;
                            } else if k == "ps-t1" {
                                has_ps_t1 = true;
                            } else if k == "ps-t2" {
                                has_ps_t2 = true;
                            }

                            if k.contains("normal")
                                || v.contains("normal")
                                || v.contains("normalmap")
                            {
                                has_normal_map = true;
                            }

                            if k == "run" {
                                if v.contains(r"commandlist\global\orfix\orfix")
                                    || v.contains("commandlist/global/orfix/orfix")
                                {
                                    has_orfix_call = true;
                                }
                                if v.contains(r"commandlist\texfx\transparency")
                                    || v.contains("commandlist/texfx/transparency")
                                {
                                    texfx_transparency_pos = Some(idx);
                                }
                            }
                        }
                        IniItem::Command { command, args } => {
                            if command.eq_ignore_ascii_case("run") {
                                for arg in args {
                                    let a_lower = arg.to_lowercase();
                                    if a_lower.contains(r"commandlist\global\orfix\orfix")
                                        || a_lower.contains("commandlist/global/orfix/orfix")
                                    {
                                        has_orfix_call = true;
                                    }
                                    if a_lower.contains(r"commandlist\texfx\transparency")
                                        || a_lower.contains("commandlist/texfx/transparency")
                                    {
                                        texfx_transparency_pos = Some(idx);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                let binds_normal_or_triple =
                    has_normal_map || (has_ps_t0 && has_ps_t1 && has_ps_t2);

                if binds_normal_or_triple && !has_orfix_call {
                    let orfix_item = IniItem::Pair {
                        key: "run".to_string(),
                        value: r"CommandList\global\ORFix\ORFix".to_string(),
                    };

                    if let Some(pos) = texfx_transparency_pos {
                        // Ensure ORFix is inserted before TexFx\Transparency
                        section.items.insert(pos, orfix_item);
                    } else {
                        section.items.push(orfix_item);
                    }
                    orfix_fixes += 1;
                }
            }
        }
    }

    if hash_fixes == 0 && slot_fixes == 0 && orfix_fixes == 0 {
        return Ok(None);
    }

    let new_content = IniCompiler::default().serialize(&doc);
    Ok(Some(TransformResult {
        new_content,
        hash_fixes,
        slot_fixes,
        orfix_fixes,
    }))
}

fn check_noapplier(mod_root: &Path, file_path: &Path, content: &str) -> bool {
    // Check root marker files
    if mod_root.join("NOAPPLIER").exists()
        || mod_root.join("noapplier").exists()
        || mod_root.join("NOAPPLIER.txt").exists()
        || mod_root.join("noapplier.txt").exists()
    {
        return true;
    }

    // Check sibling marker file
    if let Some(parent) = file_path.parent() {
        if parent.join("NOAPPLIER").exists() || parent.join("NOAPPLIER.txt").exists() {
            return true;
        }
    }

    // Check file content
    content.to_uppercase().contains("NOAPPLIER")
}

/// Previews patch fixes for a mod without modifying files on disk.
pub fn preview_mod_fix(mod_root: &Path, rules: &FixRuleSet) -> Result<FixPreview> {
    if !mod_root.exists() {
        return Err(anyhow!("Mod root does not exist: {:?}", mod_root));
    }

    let mut preview = FixPreview::default();

    for entry in WalkDir::new(mod_root)
        .into_iter()
        .filter_entry(|e| {
            // Ignore .yago_backups directory
            let name = e.file_name().to_string_lossy();
            name != ".yago_backups"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name_lower = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_ini = name_lower.ends_with(".ini") || name_lower.ends_with(".ini.yago_source");
        if !is_ini {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        let rel_path = path
            .strip_prefix(mod_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        if check_noapplier(mod_root, path, &content) {
            preview.skipped_noapplier.push(rel_path);
            continue;
        }

        if let Ok(Some(transformed)) = transform_ini_content(&content, rules) {
            let diff = generate_unified_diff(&content, &transformed.new_content);
            let changes_count =
                transformed.hash_fixes + transformed.slot_fixes + transformed.orfix_fixes;

            preview.total_hash_fixes += transformed.hash_fixes;
            preview.total_slot_fixes += transformed.slot_fixes;
            preview.total_orfix_fixes += transformed.orfix_fixes;

            preview.file_diffs.push(FileFixDiff {
                relative_path: rel_path,
                diff,
                changes_count,
            });
        }
    }

    Ok(preview)
}

/// Transactionally applies patch fixes, creating a backup folder before writing.
pub fn apply_mod_fix(mod_root: &Path, rules: &FixRuleSet) -> Result<FixApplyReport> {
    if !mod_root.exists() {
        return Err(anyhow!("Mod root does not exist: {:?}", mod_root));
    }

    // 1. Gather all files to transform
    let mut transformations = Vec::new();
    let mut total_hash_fixes = 0usize;
    let mut total_slot_fixes = 0usize;
    let mut total_orfix_fixes = 0usize;

    for entry in WalkDir::new(mod_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".yago_backups"
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path().to_path_buf();
        if !path.is_file() {
            continue;
        }

        let name_lower = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_ini = name_lower.ends_with(".ini") || name_lower.ends_with(".ini.yago_source");
        if !is_ini {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        if check_noapplier(mod_root, &path, &content) {
            continue;
        }

        if let Ok(Some(transformed)) = transform_ini_content(&content, rules) {
            total_hash_fixes += transformed.hash_fixes;
            total_slot_fixes += transformed.slot_fixes;
            total_orfix_fixes += transformed.orfix_fixes;

            let rel_path = path.strip_prefix(mod_root).unwrap_or(&path).to_path_buf();

            transformations.push((path, rel_path, transformed.new_content));
        }
    }

    if transformations.is_empty() {
        return Ok(FixApplyReport::default());
    }

    // 2. Create backup folder with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_dir = mod_root.join(".yago_backups").join(&timestamp);
    std::fs::create_dir_all(&backup_dir)?;

    // 3. Backup and overwrite
    let files_modified = transformations.len();
    for (orig_path, rel_path, new_content) in transformations {
        let backup_dest = backup_dir.join(&rel_path);
        if let Some(parent) = backup_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&orig_path, &backup_dest)?;
        std::fs::write(&orig_path, new_content)?;
    }

    Ok(FixApplyReport {
        backup_path: backup_dir.to_string_lossy().to_string(),
        files_modified,
        total_hash_fixes,
        total_slot_fixes,
        total_orfix_fixes,
    })
}

/// Restores files from the most recent backup folder under .yago_backups/
/// and removes that backup folder once restored.
pub fn rollback_mod_fix(mod_root: &Path) -> Result<usize> {
    let backups_dir = mod_root.join(".yago_backups");
    if !backups_dir.exists() {
        return Err(anyhow!("No backup directory found for this mod"));
    }

    let mut backup_entries: Vec<PathBuf> = std::fs::read_dir(&backups_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    if backup_entries.is_empty() {
        return Err(anyhow!("No backup folders available to restore"));
    }

    // Sort descending by directory name (timestamp)
    backup_entries.sort();
    let latest_backup = backup_entries.last().unwrap().clone();

    let mut restored_count = 0usize;
    for entry in WalkDir::new(&latest_backup)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let rel = path.strip_prefix(&latest_backup)?;
            let target = mod_root.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target)?;
            restored_count += 1;
        }
    }

    // Remove the restored backup folder
    let _ = std::fs::remove_dir_all(&latest_backup);

    Ok(restored_count)
}

/// Checks if at least one backup exists under mod_root/.yago_backups/
pub fn has_fix_backup(mod_root: &Path) -> bool {
    let backups_dir = mod_root.join(".yago_backups");
    if !backups_dir.exists() {
        return false;
    }

    if let Ok(entries) = std::fs::read_dir(backups_dir) {
        entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir())
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hash_migration_and_slot_remap() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let content = r#"[TextureOverrideBody]
hash = deadbeef
ps-t0 = ResourceDiffuse
ps-t1 = ResourceLightMap
"#;
        std::fs::write(&mod_ini, content).unwrap();

        let mut hash_migrations = HashMap::new();
        hash_migrations.insert("deadbeef".to_string(), "cafebabe".to_string());

        let rules = FixRuleSet {
            game_id: "genshin".to_string(),
            hash_migrations,
            slot_remaps: vec![SlotRemapRule {
                section_contains: "".to_string(),
                old_key: "ps-t0".to_string(),
                new_key: "ps-t2".to_string(),
            }],
            apply_orfix: false,
            face_fix_ps_t0_to_this: false,
            ..Default::default()
        };

        let preview = preview_mod_fix(dir.path(), &rules).unwrap();
        assert_eq!(preview.total_hash_fixes, 1);
        assert_eq!(preview.total_slot_fixes, 1);
        assert_eq!(preview.file_diffs.len(), 1);

        let diff = &preview.file_diffs[0].diff;
        assert!(diff.contains("- hash = deadbeef"));
        assert!(diff.contains("+ hash = cafebabe"));
        assert!(diff.contains("+ ps-t2 = ResourceDiffuse"));
    }

    #[test]
    fn test_noapplier_skip() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let content = r#"; NOAPPLIER marker
[TextureOverrideBody]
hash = deadbeef
ps-t0 = ResourceDiffuse
"#;
        std::fs::write(&mod_ini, content).unwrap();

        let mut hash_migrations = HashMap::new();
        hash_migrations.insert("deadbeef".to_string(), "cafebabe".to_string());

        let rules = FixRuleSet {
            game_id: "genshin".to_string(),
            hash_migrations,
            ..Default::default()
        };

        let preview = preview_mod_fix(dir.path(), &rules).unwrap();
        assert_eq!(preview.total_hash_fixes, 0);
        assert_eq!(preview.skipped_noapplier.len(), 1);
        assert!(preview.skipped_noapplier[0].contains("mod.ini"));
    }

    #[test]
    fn test_face_and_normalmap_orfix_ordering() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let content = r#"[TextureOverrideFaceHeadDiffuse]
hash = face1234
ps-t0 = ResourceFaceDiffuse

[TextureOverrideBody]
hash = body1234
ps-t0 = ResourceDiffuse
ps-t1 = ResourceNormalMap
run = CommandList\TexFx\Transparency
"#;
        std::fs::write(&mod_ini, content).unwrap();

        let rules = FixRuleSet {
            game_id: "genshin".to_string(),
            apply_orfix: true,
            face_fix_ps_t0_to_this: true,
            ..Default::default()
        };

        let preview = preview_mod_fix(dir.path(), &rules).unwrap();
        assert_eq!(preview.total_orfix_fixes, 1);
        assert_eq!(preview.total_slot_fixes, 1); // Face ps-t0 remapped to this

        let diff = &preview.file_diffs[0].diff;
        // Face head diffuse should not have ORFix injected
        assert!(!diff.contains(r"[TextureOverrideFaceHeadDiffuse]"));
        // Face head diffuse should have ps-t0 = this
        assert!(diff.contains("+ ps-t0 = this"));
        // Body normal map should have ORFix before TexFx\Transparency
        assert!(diff.contains(r"+ run = CommandList\global\ORFix\ORFix"));
    }

    #[test]
    fn test_dry_run_leaves_files_untouched() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let original_content = "[TextureOverrideBody]\nhash = deadbeef\n";
        std::fs::write(&mod_ini, original_content).unwrap();

        let mut hash_migrations = HashMap::new();
        hash_migrations.insert("deadbeef".to_string(), "12345678".to_string());

        let rules = FixRuleSet {
            game_id: "genshin".to_string(),
            hash_migrations,
            ..Default::default()
        };

        let _ = preview_mod_fix(dir.path(), &rules).unwrap();
        let read_back = std::fs::read_to_string(&mod_ini).unwrap();
        assert_eq!(read_back, original_content, "Dry-run must not modify files");
        assert!(!has_fix_backup(dir.path()));
    }

    #[test]
    fn test_apply_and_rollback_restores_exact_original() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let original_content = "[TextureOverrideBody]\nhash = deadbeef\nps-t0 = ResourceDiffuse\n";
        std::fs::write(&mod_ini, original_content).unwrap();

        let mut hash_migrations = HashMap::new();
        hash_migrations.insert("deadbeef".to_string(), "99999999".to_string());

        let rules = FixRuleSet {
            game_id: "genshin".to_string(),
            hash_migrations,
            ..Default::default()
        };

        // Apply fix
        assert!(!has_fix_backup(dir.path()));
        let report = apply_mod_fix(dir.path(), &rules).unwrap();
        assert_eq!(report.files_modified, 1);
        assert!(has_fix_backup(dir.path()));

        let modified_content = std::fs::read_to_string(&mod_ini).unwrap();
        assert!(modified_content.contains("99999999"));

        // Rollback fix
        let restored = rollback_mod_fix(dir.path()).unwrap();
        assert_eq!(restored, 1);
        assert!(!has_fix_backup(dir.path()));

        let restored_content = std::fs::read_to_string(&mod_ini).unwrap();
        assert_eq!(restored_content, original_content);
    }
}
