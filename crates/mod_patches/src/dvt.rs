use crate::error::Result;
use ini::IniCompiler;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DvtSyncReport {
    pub updated_draws: usize,
    pub copied_buffers: usize,
    pub backup_dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct ExportedDrawEntry {
    drawindexed: Option<String>,
    draw: Option<String>,
}

/// Synchronizes draw counts (drawindexed / draw) and replaces binary vertex/index buffers (.buf / .ib)
/// from a Blender 3DMigoto export into an existing mod directory without clobbering toggle logic or comments.
pub fn sync_draw_values(
    target_mod_root: &Path,
    blender_export_dir: &Path,
) -> Result<DvtSyncReport> {
    if !target_mod_root.exists() {
        return Err(crate::error::WeaverError::Logic(format!(
            "Target mod root does not exist: {:?}",
            target_mod_root
        )));
    }
    if !blender_export_dir.exists() {
        return Err(crate::error::WeaverError::Logic(format!(
            "Blender export directory does not exist: {:?}",
            blender_export_dir
        )));
    }

    // 1. Parse exported .ini files in blender_export_dir to index hash -> (drawindexed, draw)
    // Map key: (hash_lowercase, Option<match_first_index_lowercase>)
    let mut export_index: HashMap<(String, Option<String>), ExportedDrawEntry> = HashMap::new();
    let ini_compiler = IniCompiler::default();

    for entry in WalkDir::new(blender_export_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name_low = entry.file_name().to_string_lossy().to_lowercase();
            if name_low.ends_with(".ini") {
                if let Ok(doc) = ini_compiler.compile(entry.path()) {
                    for section in doc.sections {
                        if section.name.to_lowercase().starts_with("textureoverride") {
                            let mut hash = None;
                            let mut match_first = None;
                            let mut drawindexed = None;
                            let mut draw = None;

                            for item in section.items {
                                if let ini::ast::IniItem::Pair { key, value } = item {
                                    let k = key.trim().to_lowercase();
                                    let v = value.trim().to_string();
                                    if k == "hash" {
                                        hash = Some(v.to_lowercase());
                                    } else if k == "match_first_index" {
                                        match_first = Some(v.to_lowercase());
                                    } else if k == "drawindexed" {
                                        drawindexed = Some(v);
                                    } else if k == "draw" {
                                        draw = Some(v);
                                    }
                                }
                            }

                            if let Some(h) = hash {
                                let entry = export_index
                                    .entry((h.clone(), match_first.clone()))
                                    .or_default();
                                if drawindexed.is_some() {
                                    entry.drawindexed = drawindexed.clone();
                                }
                                if draw.is_some() {
                                    entry.draw = draw.clone();
                                }

                                // Also insert without match_first as fallback
                                let fallback = export_index.entry((h, None)).or_default();
                                if drawindexed.is_some() && fallback.drawindexed.is_none() {
                                    fallback.drawindexed = drawindexed;
                                }
                                if draw.is_some() && fallback.draw.is_none() {
                                    fallback.draw = draw;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Prepare timestamped backup directory compatible with fixer.rs / 1-click rollback
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_dir = target_mod_root.join(".yago_backups").join(&timestamp);
    let mut backed_up: HashSet<PathBuf> = HashSet::new();

    let backup_file_fn = |src_file: &Path, backed: &mut HashSet<PathBuf>| -> Result<()> {
        if !backed.contains(src_file) && src_file.exists() {
            let rel = src_file.strip_prefix(target_mod_root).unwrap_or(src_file);
            let dest = backup_dir.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src_file, &dest)?;
            backed.insert(src_file.to_path_buf());
        }
        Ok(())
    };

    // 3. Copy updated .buf and .ib files from blender_export_dir to target_mod_root
    let mut copied_buffers = 0usize;
    for entry in WalkDir::new(blender_export_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name_low = entry.file_name().to_string_lossy().to_lowercase();
            if name_low.ends_with(".buf") || name_low.ends_with(".ib") {
                let file_name = entry.file_name();
                let rel = entry
                    .path()
                    .strip_prefix(blender_export_dir)
                    .unwrap_or(entry.path());

                // Find matching destination in target_mod_root:
                // 1) exact relative path
                // 2) or find file with matching filename anywhere in target_mod_root
                let direct_target = target_mod_root.join(rel);
                let target_dest = if direct_target.exists() {
                    direct_target
                } else {
                    let mut found = None;
                    for t_entry in WalkDir::new(target_mod_root)
                        .into_iter()
                        .filter_entry(|e| e.file_name() != ".yago_backups")
                        .filter_map(|e| e.ok())
                    {
                        if t_entry.file_type().is_file()
                            && t_entry
                                .file_name()
                                .to_string_lossy()
                                .eq_ignore_ascii_case(&file_name.to_string_lossy())
                        {
                            found = Some(t_entry.path().to_path_buf());
                            break;
                        }
                    }
                    found.unwrap_or(direct_target)
                };

                backup_file_fn(&target_dest, &mut backed_up)?;

                if let Some(parent) = target_dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(entry.path(), &target_dest)?;
                copied_buffers += 1;
            }
        }
    }

    // 4. Update drawindexed / draw lines in target .ini and .ini.yago_source files
    let mut updated_draws = 0usize;

    for entry in WalkDir::new(target_mod_root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".yago_backups")
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let name_low = entry.file_name().to_string_lossy().to_lowercase();
            if name_low.ends_with(".ini") || name_low.ends_with(".ini.yago_source") {
                let path = entry.path();
                let Ok(content) = std::fs::read_to_string(path) else {
                    continue;
                };

                let (modified_content, count) = update_ini_draw_values(&content, &export_index);

                if count > 0 {
                    backup_file_fn(path, &mut backed_up)?;
                    std::fs::write(path, modified_content)?;
                    updated_draws += count;
                }
            }
        }
    }

    Ok(DvtSyncReport {
        updated_draws,
        copied_buffers,
        backup_dir,
    })
}

/// Updates drawindexed / draw values inside matching [TextureOverride*] sections
/// while strictly preserving line format, indentation, comments, and if/elif/else/endif logic.
fn update_ini_draw_values(
    content: &str,
    export_index: &HashMap<(String, Option<String>), ExportedDrawEntry>,
) -> (String, usize) {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let mut total_updated = 0usize;

    // First pass: locate section ranges and their metadata
    struct SectionRange {
        header_name: String,
        start_line: usize,
        end_line: usize,
        hash: Option<String>,
        match_first: Option<String>,
    }

    let mut sections = Vec::new();
    let mut current_sec_name = None;
    let mut current_sec_start = 0;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(sec_name) = current_sec_name {
                sections.push(SectionRange {
                    header_name: sec_name,
                    start_line: current_sec_start,
                    end_line: idx,
                    hash: None,
                    match_first: None,
                });
            }
            current_sec_name = Some(trimmed[1..trimmed.len() - 1].trim().to_string());
            current_sec_start = idx;
        }
    }

    if let Some(sec_name) = current_sec_name {
        sections.push(SectionRange {
            header_name: sec_name,
            start_line: current_sec_start,
            end_line: lines.len(),
            hash: None,
            match_first: None,
        });
    }

    // Inspect each section for hash and match_first_index
    for sec in &mut sections {
        if sec
            .header_name
            .to_lowercase()
            .starts_with("textureoverride")
        {
            for line_idx in sec.start_line..sec.end_line {
                let trimmed = lines[line_idx].trim();
                if let Some(eq_pos) = trimmed.find('=') {
                    let k = trimmed[..eq_pos].trim().to_lowercase();
                    let v = trimmed[eq_pos + 1..].trim();
                    // Strip inline comment if present
                    let val = v.split(';').next().unwrap_or(v).trim();
                    if k == "hash" {
                        sec.hash = Some(val.to_lowercase());
                    } else if k == "match_first_index" {
                        sec.match_first = Some(val.to_lowercase());
                    }
                }
            }
        }
    }

    // Second pass: update matching drawindexed / draw lines in matching sections
    for sec in &sections {
        if let Some(h) = &sec.hash {
            let matched_entry = export_index
                .get(&(h.clone(), sec.match_first.clone()))
                .or_else(|| export_index.get(&(h.clone(), None)));

            if let Some(entry) = matched_entry {
                for line_idx in sec.start_line..sec.end_line {
                    let line = &lines[line_idx];

                    if let Some(eq_pos) = line.find('=') {
                        let k = line[..eq_pos].trim().to_lowercase();
                        if k == "drawindexed" {
                            if let Some(new_val) = &entry.drawindexed {
                                let leading_ws: String =
                                    line.chars().take_while(|c| c.is_whitespace()).collect();
                                let mut comment = "";
                                if let Some(semi_pos) = line.find(';') {
                                    comment = &line[semi_pos..];
                                }
                                lines[line_idx] = if comment.is_empty() {
                                    format!("{}drawindexed = {}", leading_ws, new_val)
                                } else {
                                    format!(
                                        "{}drawindexed = {}    {}",
                                        leading_ws, new_val, comment
                                    )
                                };
                                total_updated += 1;
                            }
                        } else if k == "draw" {
                            if let Some(new_val) = &entry.draw {
                                let leading_ws: String =
                                    line.chars().take_while(|c| c.is_whitespace()).collect();
                                let mut comment = "";
                                if let Some(semi_pos) = line.find(';') {
                                    comment = &line[semi_pos..];
                                }
                                lines[line_idx] = if comment.is_empty() {
                                    format!("{}draw = {}", leading_ws, new_val)
                                } else {
                                    format!("{}draw = {}    {}", leading_ws, new_val, comment)
                                };
                                total_updated += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let trailing_newline = if content.ends_with('\n') { "\n" } else { "" };
    (
        format!("{}{}", lines.join("\n"), trailing_newline),
        total_updated,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sync_draw_values_preserves_toggle_logic_and_copies_buffers() {
        let dir = tempdir().unwrap();
        let target_mod = dir.path().join("TargetMod");
        let blender_export = dir.path().join("BlenderExport");

        std::fs::create_dir_all(&target_mod).unwrap();
        std::fs::create_dir_all(&blender_export).unwrap();

        // 1. Create target mod with complex toggle logic & custom comments
        let target_ini_path = target_mod.join("mod.ini");
        let target_ini_content = r#"; Keqing Custom Armor Mod
[Constants]
global $toggle = 0

[TextureOverrideKeqingBody]
hash = deadbeef
match_first_index = 0
vb0 = ResourceBodyVB
; Author comment on logic
if $toggle == 1
    drawindexed = 1000
else
    drawindexed = 1000
endif
"#;
        std::fs::write(&target_ini_path, target_ini_content).unwrap();

        // Old buffer in target
        let target_buf = target_mod.join("Body.buf");
        std::fs::write(&target_buf, b"OLD_BUFFER_DATA").unwrap();

        // 2. Create newly exported Blender mod with new draw count and new buffer data
        let export_ini_path = blender_export.join("Exported.ini");
        let export_ini_content = r#"
[TextureOverrideKeqingBody]
hash = deadbeef
match_first_index = 0
ib = ResourceBodyIB
vb0 = ResourceBodyVB
drawindexed = 54321
"#;
        std::fs::write(&export_ini_path, export_ini_content).unwrap();

        // New buffer in blender export
        let export_buf = blender_export.join("Body.buf");
        std::fs::write(&export_buf, b"NEW_BLENDER_BUFFER_DATA").unwrap();

        // 3. Run sync_draw_values
        let report = sync_draw_values(&target_mod, &blender_export).unwrap();

        assert_eq!(report.copied_buffers, 1, "Should copy 1 buffer file");
        assert_eq!(
            report.updated_draws, 2,
            "Should update 2 drawindexed lines inside if/else"
        );

        // 4. Verify target mod.ini content
        let updated_content = std::fs::read_to_string(&target_ini_path).unwrap();
        assert!(updated_content.contains("drawindexed = 54321"));
        assert!(!updated_content.contains("drawindexed = 1000"));
        assert!(updated_content.contains("if $toggle == 1"));
        assert!(updated_content.contains("; Author comment on logic"));
        assert!(updated_content.contains("else"));
        assert!(updated_content.contains("endif"));

        // 5. Verify buffer was replaced
        let new_buf_data = std::fs::read(&target_buf).unwrap();
        assert_eq!(new_buf_data, b"NEW_BLENDER_BUFFER_DATA");

        // 6. Verify backup folder was created under .yago_backups/<timestamp>
        assert!(report.backup_dir.exists());
        let backup_ini = report.backup_dir.join("mod.ini");
        assert!(backup_ini.exists());
        let backup_content = std::fs::read_to_string(backup_ini).unwrap();
        assert_eq!(backup_content, target_ini_content);

        let backup_buf = report.backup_dir.join("Body.buf");
        assert!(backup_buf.exists());
        assert_eq!(std::fs::read(backup_buf).unwrap(), b"OLD_BUFFER_DATA");
    }
}
