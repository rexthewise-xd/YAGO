use crate::error::Result;
use crate::merger::Merger;
use crate::namespacer::Namespacer;
use crate::ModProfile;
use ini::ast::{IniItem, Section};
use std::path::PathBuf;
use vfs::DeploymentPlan;

pub struct CompiledGroup {
    pub character_name: String,
    pub plan: DeploymentPlan,
}

pub struct CycleCompiler;

impl CycleCompiler {
    /// Compiles a group of mods for a single character into an isolated virtual structure.
    pub fn compile_character_group(
        character_name: &str,
        mods: Vec<ModProfile>,
    ) -> Result<CompiledGroup> {
        let mut plan = DeploymentPlan {
            symlink_map: Vec::new(),
            generated_files: Vec::new(),
            state_hash: "".to_string(),
        };

        let compiler = ini::IniCompiler::default();
        let mut all_sections = Vec::new();
        let char_normalized = character_name.to_lowercase().replace(' ', "_");
        let var_name = format!("wardrobe_{}", char_normalized);
        let active_var_name = format!("wardrobe_active_{}", char_normalized);

        // 1. Unified Character Constants
        let constants = Section {
            name: "Constants".to_string(),
            items: vec![
                IniItem::Pair {
                    key: format!("global persist ${}", var_name),
                    value: "0".to_string(), // 0 is Vanilla
                },
                IniItem::Pair {
                    key: format!("global ${}", active_var_name),
                    value: "0".to_string(),
                },
            ],
        };
        all_sections.push(constants);

        for (i, mod_profile) in mods.iter().enumerate() {
            let skin_namespace = format!("Skin_{}", i);
            let skin_index = i + 1; // Start from 1, 0 is Vanilla

            // 1. Asset Isolation (Virtual)
            let target_subfolder = PathBuf::from(format!(
                "22_Characters/{}/{}",
                character_name, skin_namespace
            ));
            plan.symlink_map
                .push((mod_profile.mod_root.clone(), target_subfolder.clone()));

            // 2. Pre-Merge: Parse all mod INIs and combine into a single Mod AST
            let mut internal_docs = Vec::new();
            let mut all_namespaces = Vec::new();

            for ini_path in &mod_profile.ini_paths {
                if ini_path.exists() {
                    if let Ok(doc) = compiler.compile(ini_path) {
                        all_namespaces.extend(Namespacer::collect_namespaces(&doc));
                        internal_docs.push((ini_path.clone(), doc));
                    }
                }
            }

            if internal_docs.is_empty() {
                continue;
            }

            let mut processed_docs = Vec::new();
            for (path, mut doc) in internal_docs {
                let mut prefix = format!("{}/", skin_namespace);
                if let Ok(rel_path) = path.parent().unwrap().strip_prefix(&mod_profile.mod_root) {
                    if !rel_path.as_os_str().is_empty() {
                        prefix.push_str(&rel_path.to_string_lossy());
                        prefix.push('/');
                    }
                }

                Namespacer::namespace_variables(&mut doc, &mod_profile.uuid, &all_namespaces)?;
                Namespacer::rewrite_paths(&mut doc, &prefix)?;
                Namespacer::strip_namespaces(&mut doc);

                processed_docs.push(doc);
            }

            let mut unified_doc = Merger::merge_documents(processed_docs)?;

            // Apply anchors, creditinfo suppression, and preset bridge
            if !mod_profile.ini_anchors.is_empty() {
                crate::presets::apply_ini_anchors(&mut unified_doc, &mod_profile.ini_anchors);
            }
            if mod_profile.suppress_creditinfo {
                crate::presets::apply_creditinfo_suppression(&mut unified_doc);
            }
            if let Some(preset_vars) = &mod_profile.active_preset_vars {
                crate::presets::inject_preset_bridge(
                    &mut unified_doc,
                    preset_vars,
                    &mod_profile.uuid,
                );
            }

            // 3. Process Sections
            for section in &mut unified_doc.sections {
                let name_lower = section.name.to_lowercase();

                let is_key_section = (name_lower.starts_with("key") || name_lower.contains("key"))
                    && !name_lower.starts_with("textureoverride")
                    && !name_lower.starts_with("shaderoverride");

                if is_key_section {
                    // Inject Wardrobe Gate for this Key Section, preserving author's original key binding
                    Merger::inject_condition(section, &format!("${} == {}", var_name, skin_index));
                }

                // Wrap Overrides in Wardrobe Gates (Router Protocol)
                let _ = Merger::wrap_in_wardrobe_gate(section, &var_name, skin_index);

                // Add collision-free on-screen character tracking ($wardrobe_active_{char} = 1)
                // Inside each [TextureOverride*] section (after declarative headers, outside the if $wardrobe_{char} == {skin_index} gate so Vanilla 0 is also tracked)
                if name_lower.starts_with("textureoverride") {
                    let insert_idx = section
                        .items
                        .iter()
                        .position(|item| match item {
                            IniItem::Command { command, .. } => command.eq_ignore_ascii_case("if"),
                            _ => false,
                        })
                        .unwrap_or(section.items.len());

                    let active_key = format!("${}", active_var_name);
                    let already_has_active = section.items.iter().any(|item| {
                        if let IniItem::Pair { key, .. } = item {
                            key.eq_ignore_ascii_case(&active_key)
                        } else {
                            false
                        }
                    });

                    if !already_has_active {
                        section.items.insert(
                            insert_idx,
                            IniItem::Pair {
                                key: active_key,
                                value: "1".to_string(),
                            },
                        );
                    }
                }

                all_sections.push(section.clone());
            }
        }

        // 3. Inject Present Section (Frame Reset for On-Screen Tracking)
        let present = Section {
            name: "Present".to_string(),
            items: vec![IniItem::Pair {
                key: format!("post ${}", active_var_name),
                value: "0".to_string(),
            }],
        };
        all_sections.push(present);

        // 4. Inject Cycle Logic (Symmetrical Keys: [ and ] )
        let cycle_vals = (0..=mods.len())
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let mut cycle = Section {
            name: format!("KeyCycle{}", character_name.replace(' ', "")),
            items: vec![
                IniItem::Pair {
                    key: "key".to_string(),
                    value: "]".to_string(), // Forward
                },
                IniItem::Pair {
                    key: "back".to_string(),
                    value: "[".to_string(), // Backward
                },
                IniItem::Pair {
                    key: "type".to_string(),
                    value: "cycle".to_string(),
                },
                IniItem::Pair {
                    key: format!("${}", var_name),
                    value: cycle_vals,
                },
            ],
        };
        Merger::inject_condition(&mut cycle, &format!("${} == 1", active_var_name));
        all_sections.push(cycle);

        // 5. Generate the Final Merged Character INI
        let final_doc = Merger::merge_documents(vec![ini::ast::IniDocument {
            sections: all_sections,
        }])?;

        let output = compiler.serialize(&final_doc);

        plan.generated_files.push((
            PathBuf::from(format!("22_Characters/{}/merged.ini", character_name)),
            output,
        ));

        Ok(CompiledGroup {
            character_name: character_name.to_string(),
            plan,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_compile_character_group_cycle_and_active_tracking() {
        let dir = tempdir().unwrap();

        let mod1_dir = dir.path().join("mod1");
        fs::create_dir_all(&mod1_dir).unwrap();
        let mod1_ini = mod1_dir.join("keqing_skin1.ini");
        let mod1_content = r#"
[KeyToggle]
key = k
type = toggle
$skin_toggle = 0, 1

[TextureOverrideBody]
hash = deadbeef
match_first_index = 0
vb0 = ResourceVB0
ps-t0 = ResourceDiffuse
"#;
        fs::write(&mod1_ini, mod1_content).unwrap();

        let mod2_dir = dir.path().join("mod2");
        fs::create_dir_all(&mod2_dir).unwrap();
        let mod2_ini = mod2_dir.join("keqing_skin2.ini");
        let mod2_content = r#"
[KeyAltToggle]
key = j
type = toggle
$alt_toggle = 0, 1

[TextureOverrideBody]
hash = deadbeef
match_first_index = 0
vb0 = ResourceAltVB0
ps-t0 = ResourceAltDiffuse
"#;
        fs::write(&mod2_ini, mod2_content).unwrap();

        let mod1 = ModProfile {
            uuid: "11111111-1111-1111-1111-111111111111".to_string(),
            mod_root: mod1_dir,
            ini_paths: vec![mod1_ini],
            deploy_path: "22_Characters/Keqing/Skin_0".to_string(),
            character: "Keqing".to_string(),
            mod_type: "character".to_string(),
            nsfw: false,
            tags: vec![],
            active_preset_vars: None,
            ini_anchors: vec![],
            suppress_creditinfo: false,
        };

        let mod2 = ModProfile {
            uuid: "22222222-2222-2222-2222-222222222222".to_string(),
            mod_root: mod2_dir,
            ini_paths: vec![mod2_ini],
            deploy_path: "22_Characters/Keqing/Skin_1".to_string(),
            character: "Keqing".to_string(),
            mod_type: "character".to_string(),
            nsfw: false,
            tags: vec![],
            active_preset_vars: None,
            ini_anchors: vec![],
            suppress_creditinfo: false,
        };

        let compiled = CycleCompiler::compile_character_group("Keqing", vec![mod1, mod2]).unwrap();

        let (_, ini_content) = compiled
            .plan
            .generated_files
            .iter()
            .find(|(path, _)| path.ends_with("merged.ini"))
            .expect("merged.ini must be generated");

        // 1. Verify [KeyCycle*] emits $wardrobe_{char} = 0, 1, 2 (and does not emit val = or curr_val =)
        assert!(ini_content.contains("$wardrobe_keqing = 0, 1, 2"));
        assert!(!ini_content.contains("val ="));
        assert!(!ini_content.contains("curr_val ="));

        // 2. Verify condition = $wardrobe_active_{char} == 1 is present on [KeyCycle*]
        assert!(ini_content.contains("condition = $wardrobe_active_keqing == 1"));

        // 3. Verify post $wardrobe_active_{char} = 0 is in [Present]
        assert!(ini_content.contains("post $wardrobe_active_keqing = 0"));

        // 4. Verify $wardrobe_active_{char} = 1 is outside the wardrobe if gate in [TextureOverride*]
        let active_pos = ini_content
            .find("$wardrobe_active_keqing = 1")
            .expect("$wardrobe_active_keqing = 1 must be present");
        let if_pos = ini_content
            .find("if $wardrobe_keqing == 1")
            .expect("if $wardrobe_keqing == 1 must be present");
        assert!(
            active_pos < if_pos,
            "$wardrobe_active_keqing = 1 must appear before wardrobe if gate"
        );

        // 5. Verify the mod's original key = k is preserved and gated with $wardrobe_{char} == 1
        assert!(ini_content.contains("key = k"));
        assert!(ini_content.contains("condition = $wardrobe_keqing == 1"));

        // 6. Verify serialized merged.ini parses cleanly back through parse_ini and passes Validator::validate_logic
        let (_, doc) = ini::parser::parse_ini(ini_content).expect("merged.ini must parse cleanly");
        Validator::validate_logic(&doc).expect("merged.ini must pass Validator::validate_logic");
    }
}
