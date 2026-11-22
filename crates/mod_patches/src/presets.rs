use anyhow::{anyhow, Result};
use ini::ast::{IniDocument, IniItem, Section};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct IniAnchor {
    pub section: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct VariantPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

fn is_transient_variable(var_name: &str) -> bool {
    let lower = var_name.to_lowercase();
    lower.contains("menu")
        || lower.contains("page")
        || lower.contains("ui_open")
        || lower.contains("creditinfo")
        || lower.starts_with("yago_")
        || lower.starts_with("wardrobe_")
}

/// Scans the mod's .ini / .ini.yago_source [Constants] sections for
/// global persist $var = <val> declarations and builds an immutable "Default" preset.
pub fn extract_default_preset(mod_root: &Path) -> VariantPreset {
    let mut variables = HashMap::new();
    if !mod_root.exists() {
        return VariantPreset {
            id: "default".to_string(),
            name: "Default".to_string(),
            is_default: true,
            variables,
        };
    }

    for entry in WalkDir::new(mod_root)
        .into_iter()
        .filter_entry(|e| e.file_name().to_string_lossy() != ".yago_backups")
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

        let Ok((_, doc)) = ini::parser::parse_ini(&content) else {
            continue;
        };

        for section in &doc.sections {
            if section.name.eq_ignore_ascii_case("Constants") {
                for item in &section.items {
                    if let IniItem::Pair { key, value } = item {
                        let key_trim = key.trim();
                        // Look for global persist $var = ... or global $var = ...
                        let parts: Vec<&str> = key_trim.split_whitespace().collect();
                        let var_token = parts.iter().find(|p| p.starts_with('$'));
                        if let Some(token) = var_token {
                            let clean_name = token.trim_start_matches('$');
                            if !is_transient_variable(clean_name) {
                                variables
                                    .entry(clean_name.to_string())
                                    .or_insert_with(|| value.trim().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    VariantPreset {
        id: "default".to_string(),
        name: "Default".to_string(),
        is_default: true,
        variables,
    }
}

/// Parses [Constants] in d3dx_user.ini and extracts current values for persisted variables
/// matching this mod's namespace / variable names.
pub fn capture_from_d3dx_user(
    d3dx_user_path: &Path,
    mod_namespace_prefix: &str,
    mod_vars: &[String],
) -> Result<HashMap<String, String>> {
    let mut captured = HashMap::new();
    if !d3dx_user_path.exists() {
        return Ok(captured);
    }

    let content = std::fs::read_to_string(d3dx_user_path)?;
    let (_, doc) = ini::parser::parse_ini(&content)
        .map_err(|e| anyhow!("Failed to parse d3dx_user.ini: {:?}", e))?;

    let prefix_lower = mod_namespace_prefix.to_lowercase().replace('-', "_");

    for section in &doc.sections {
        if section.name.eq_ignore_ascii_case("Constants") {
            for item in &section.items {
                if let IniItem::Pair { key, value } = item {
                    let key_clean = key.trim().trim_start_matches('$');
                    let key_lower = key_clean.to_lowercase();

                    // 1. Exact match against known mod variables
                    for v in mod_vars {
                        let v_clean = v.trim_start_matches('$');
                        if key_clean.eq_ignore_ascii_case(v_clean) {
                            captured.insert(v_clean.to_string(), value.trim().to_string());
                        }
                    }

                    // 2. Namespace matching: variable ends with _<namespace> or contains namespace
                    if !prefix_lower.is_empty() && key_lower.contains(&prefix_lower) {
                        let base_name =
                            if let Some(idx) = key_lower.rfind(&format!("_{}", prefix_lower)) {
                                &key_clean[..idx]
                            } else {
                                key_clean
                            };
                        if !is_transient_variable(base_name) {
                            captured.insert(base_name.to_string(), value.trim().to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(captured)
}

/// Generates a ModDeck-style one-frame [Present] bridge:
/// Declares global $yago_preset_applied_{unique_id} = 0 in [Constants].
/// In [Present], emits:
/// if $yago_preset_applied_{unique_id} == 0
///     $var1 = val1
///     ...
///     $yago_preset_applied_{unique_id} = 1
/// endif
pub fn inject_preset_bridge(
    doc: &mut IniDocument,
    preset_vars: &HashMap<String, String>,
    unique_id: &str,
) {
    if preset_vars.is_empty() {
        return;
    }

    let safe_id = unique_id.replace('-', "_");
    let flag_var = format!("$yago_preset_applied_{}", safe_id);

    // 1. Declare flag in [Constants]
    let constants_sec = if let Some(sec) = doc
        .sections
        .iter_mut()
        .find(|s| s.name.eq_ignore_ascii_case("Constants"))
    {
        sec
    } else {
        doc.sections.insert(
            0,
            Section {
                name: "Constants".to_string(),
                items: Vec::new(),
            },
        );
        &mut doc.sections[0]
    };

    let flag_decl = format!("global {}", flag_var);
    if !constants_sec.items.iter().any(|item| {
        if let IniItem::Pair { key, .. } = item {
            key.eq_ignore_ascii_case(&flag_decl)
        } else {
            false
        }
    }) {
        constants_sec.items.push(IniItem::Pair {
            key: flag_decl,
            value: "0".to_string(),
        });
    }

    // 2. Map preset variable names to actual declared variable names in doc
    let declared_vars: Vec<String> = constants_sec
        .items
        .iter()
        .filter_map(|item| {
            if let IniItem::Pair { key, .. } = item {
                let parts: Vec<&str> = key.split_whitespace().collect();
                parts
                    .iter()
                    .find(|p| p.starts_with('$'))
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut actual_var_assignments = Vec::new();

    for (k, v) in preset_vars {
        let clean_k = k.trim_start_matches('$');
        let matched_var = declared_vars.iter().find(|decl| {
            let decl_clean = decl.trim_start_matches('$');
            decl_clean.eq_ignore_ascii_case(clean_k)
                || decl_clean
                    .to_lowercase()
                    .starts_with(&format!("{}_", clean_k.to_lowercase()))
        });

        let target_var_name = if let Some(decl) = matched_var {
            decl.clone()
        } else {
            format!("${}", clean_k)
        };

        actual_var_assignments.push((target_var_name, v.clone()));
    }

    actual_var_assignments.sort_by(|a, b| a.0.cmp(&b.0));

    // 3. Inject one-frame gate in [Present]
    let present_sec = if let Some(sec) = doc
        .sections
        .iter_mut()
        .find(|s| s.name.eq_ignore_ascii_case("Present"))
    {
        sec
    } else {
        doc.sections.push(Section {
            name: "Present".to_string(),
            items: Vec::new(),
        });
        doc.sections.last_mut().unwrap()
    };

    present_sec.items.push(IniItem::Command {
        command: "if".to_string(),
        args: vec![format!("{} == 0", flag_var)],
    });

    for (var_name, val) in actual_var_assignments {
        present_sec.items.push(IniItem::Pair {
            key: var_name,
            value: val,
        });
    }

    present_sec.items.push(IniItem::Pair {
        key: flag_var,
        value: "1".to_string(),
    });

    present_sec.items.push(IniItem::Command {
        command: "endif".to_string(),
        args: Vec::new(),
    });
}

/// Applies user-pinned IniAnchor overrides to matching sections and keys.
pub fn apply_ini_anchors(doc: &mut IniDocument, anchors: &[IniAnchor]) {
    for anchor in anchors {
        if !anchor.enabled {
            continue;
        }

        let section = doc
            .sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&anchor.section));

        if let Some(sec) = section {
            let mut key_found = false;
            for item in &mut sec.items {
                if let IniItem::Pair { key, value } = item {
                    if key.trim().eq_ignore_ascii_case(&anchor.key) {
                        *value = anchor.value.clone();
                        key_found = true;
                        break;
                    }
                }
            }
            if !key_found {
                sec.items.push(IniItem::Pair {
                    key: anchor.key.clone(),
                    value: anchor.value.clone(),
                });
            }
        } else {
            doc.sections.push(Section {
                name: anchor.section.clone(),
                items: vec![IniItem::Pair {
                    key: anchor.key.clone(),
                    value: anchor.value.clone(),
                }],
            });
        }
    }
}

/// Suppresses on-screen $creditinfo spam by zeroing or disabling assignments.
pub fn apply_creditinfo_suppression(doc: &mut IniDocument) {
    for section in &mut doc.sections {
        for item in &mut section.items {
            if let IniItem::Pair { key, value } = item {
                let key_clean = key.trim().to_lowercase();
                if key_clean.contains("creditinfo") {
                    *value = "0".to_string();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;
    use tempfile::tempdir;

    #[test]
    fn test_extract_default_preset() {
        let dir = tempdir().unwrap();
        let mod_ini = dir.path().join("mod.ini");
        let content = r#"[Constants]
global persist $costume = 1
global persist $glasses = 0
global persist $menu_open = 1
global persist $creditinfo = 1
"#;
        std::fs::write(&mod_ini, content).unwrap();

        let preset = extract_default_preset(dir.path());
        assert!(preset.is_default);
        assert_eq!(preset.name, "Default");
        assert_eq!(
            preset.variables.get("costume").map(|s| s.as_str()),
            Some("1")
        );
        assert_eq!(
            preset.variables.get("glasses").map(|s| s.as_str()),
            Some("0")
        );
        // Transient menu and creditinfo must be filtered out
        assert!(!preset.variables.contains_key("menu_open"));
        assert!(!preset.variables.contains_key("creditinfo"));
    }

    #[test]
    fn test_capture_from_d3dx_user() {
        let dir = tempdir().unwrap();
        let d3dx_path = dir.path().join("d3dx_user.ini");
        let content = r#"[Constants]
$costume = 2
$costume_mod123 = 3
$weapon_mod123 = 1
$other_var = 5
"#;
        std::fs::write(&d3dx_path, content).unwrap();

        let known_vars = vec!["costume".to_string()];
        let captured = capture_from_d3dx_user(&d3dx_path, "mod123", &known_vars).unwrap();

        assert_eq!(
            captured.get("costume").map(|s| s.as_str()),
            Some("3").or_else(|| captured.get("costume").map(|s| s.as_str()))
        );
        assert_eq!(captured.get("weapon").map(|s| s.as_str()), Some("1"));
    }

    #[test]
    fn test_inject_preset_bridge_and_validation() {
        let content = r#"[Constants]
global persist $costume = 0
global $active = 0

[Present]
post $active = 0
"#;
        let (_, mut doc) = ini::parser::parse_ini(content).unwrap();
        let mut preset_vars = HashMap::new();
        preset_vars.insert("costume".to_string(), "2".to_string());

        inject_preset_bridge(&mut doc, &preset_vars, "uuid_123");

        let present = doc.sections.iter().find(|s| s.name == "Present").unwrap();
        let has_gate = present.items.iter().any(|item| match item {
            IniItem::Command { command, args } => {
                command == "if" && args[0].contains("$yago_preset_applied_uuid_123 == 0")
            }
            _ => false,
        });
        assert!(has_gate, "One-frame bridge gate must be present");

        // Validate logic
        let validation = Validator::validate_logic(&doc);
        assert!(
            validation.is_ok(),
            "Bridged document must pass validation: {:?}",
            validation.err()
        );
    }

    #[test]
    fn test_apply_ini_anchors_and_creditinfo() {
        let content = r#"[Constants]
global persist $creditinfo = 1

[TextureOverrideBody]
hash = deadbeef
filter_index = 0
"#;
        let (_, mut doc) = ini::parser::parse_ini(content).unwrap();

        let anchors = vec![IniAnchor {
            section: "TextureOverrideBody".to_string(),
            key: "filter_index".to_string(),
            value: "2".to_string(),
            enabled: true,
        }];

        apply_ini_anchors(&mut doc, &anchors);
        apply_creditinfo_suppression(&mut doc);

        let sec = doc
            .sections
            .iter()
            .find(|s| s.name == "TextureOverrideBody")
            .unwrap();
        let filter_val = sec.items.iter().find_map(|item| {
            if let IniItem::Pair { key, value } = item {
                if key == "filter_index" {
                    Some(value.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });
        assert_eq!(filter_val.as_deref(), Some("2"));

        let consts = doc.sections.iter().find(|s| s.name == "Constants").unwrap();
        let credit_val = consts.items.iter().find_map(|item| {
            if let IniItem::Pair { key, value } = item {
                if key.contains("creditinfo") {
                    Some(value.clone())
                } else {
                    None
                }
            } else {
                None
            }
        });
        assert_eq!(credit_val.as_deref(), Some("0"));
    }
}
