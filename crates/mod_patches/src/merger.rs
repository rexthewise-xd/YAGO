use crate::error::Result;
use ini::ast::{IniDocument, IniItem, Section};
use std::collections::{HashMap, HashSet};

pub struct Merger;

impl Merger {
    fn is_declarative_match_header(item: &IniItem) -> bool {
        if let IniItem::Pair { key, .. } = item {
            let key_lower = key.trim().to_lowercase();
            key_lower == "hash"
                || key_lower == "allow_duplicate_hash"
                || key_lower == "filter_index"
                || key_lower.starts_with("match_")
        } else {
            false
        }
    }

    fn wrap_section_items_in_gate(section: &mut Section, condition_arg: &str) {
        let (declarative, runtime): (Vec<IniItem>, Vec<IniItem>) = section
            .items
            .drain(..)
            .partition(Self::is_declarative_match_header);

        let has_runtime_body = runtime.iter().any(|item| match item {
            IniItem::Pair { .. } | IniItem::Command { .. } => true,
            IniItem::Comment(_) => false,
        });

        if !has_runtime_body {
            let mut all_items = declarative;
            all_items.extend(runtime);
            section.items = all_items;
            return;
        }

        let mut new_items = declarative;
        new_items.push(IniItem::Command {
            command: "if".to_string(),
            args: vec![condition_arg.to_string()],
        });
        new_items.extend(runtime);
        new_items.push(IniItem::Command {
            command: "endif".to_string(),
            args: vec![],
        });

        section.items = new_items;
    }

    /// Wraps a specific section in a logic gate.
    /// Used for [TextureOverride] blocks to ensure they only activate under specific conditions.
    pub fn wrap_in_logic_gate(section: &mut Section, condition_arg: &str) -> Result<()> {
        let name_lower = section.name.to_lowercase();
        if !name_lower.starts_with("textureoverride") && !name_lower.starts_with("shaderoverride") {
            return Ok(()); // Only wrap overrides
        }

        Self::wrap_section_items_in_gate(section, condition_arg);
        Ok(())
    }

    /// Injects a condition into a section (e.g. for Keybinds).
    /// If a condition exists, it appends " && new_condition".
    /// If not, it creates "condition = new_condition".
    pub fn inject_condition(section: &mut Section, new_condition: &str) {
        let mut found = false;
        for item in &mut section.items {
            if let IniItem::Pair { key, value } = item {
                if key.eq_ignore_ascii_case("condition") {
                    // Append
                    *value = format!("{} && {}", value, new_condition);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            // Insert at the top (usually better for readability/parsing)
            section.items.insert(
                0,
                IniItem::Pair {
                    key: "condition".to_string(),
                    value: new_condition.to_string(),
                },
            );
        }
    }

    /// Wraps a section in a Wardrobe logic gate.
    pub fn wrap_in_wardrobe_gate(
        section: &mut Section,
        var_name: &str,
        index: usize,
    ) -> Result<()> {
        let name_lower = section.name.to_lowercase();
        if !name_lower.starts_with("textureoverride") && !name_lower.starts_with("shaderoverride") {
            return Ok(());
        }

        let condition_arg = format!("${} == {}", var_name, index);
        Self::wrap_section_items_in_gate(section, &condition_arg);
        Ok(())
    }

    /// Merges multiple INI documents into one.
    /// Consolidates sections with the same name and removes duplicate non-command keys.
    pub fn merge_documents(docs: Vec<IniDocument>) -> Result<IniDocument> {
        let mut section_map: HashMap<String, Vec<IniItem>> = HashMap::new();
        let mut section_order: Vec<String> = Vec::new();

        for doc in docs {
            for section in doc.sections {
                if !section_map.contains_key(&section.name) {
                    section_order.push(section.name.clone());
                }
                section_map
                    .entry(section.name)
                    .or_default()
                    .extend(section.items);
            }
        }

        let mut combined_sections = Vec::new();
        for name in section_order {
            if let Some(items) = section_map.remove(&name) {
                let name_lower = name.to_lowercase();
                let is_dedupe_section = name_lower == "constants" || name_lower == "global";

                // Aggressive Deduplication within sections
                let mut deduped_items = Vec::new();
                let mut seen_keys = HashSet::new();

                for item in items {
                    match &item {
                        IniItem::Pair { key, .. } => {
                            let key_lower = key.to_lowercase();

                            // Commands/Special keys that ARE allowed to repeat:
                            // run, draw, handling, this, checktextureoverride, ps-t*, vs-t*, o*
                            let can_repeat = key_lower == "run"
                                || key_lower == "draw"
                                || key_lower == "ps-t"
                                || key_lower == "vs-t"
                                || key_lower.starts_with("ps-t")
                                || key_lower.starts_with("vs-t")
                                || key_lower.starts_with("o");

                            if !is_dedupe_section || can_repeat {
                                deduped_items.push(item);
                            } else {
                                // For globals, we deduplicate by the variable name part
                                let dedupe_key = if key_lower.starts_with("global persist ") {
                                    key_lower
                                        .strip_prefix("global persist ")
                                        .unwrap_or(&key_lower)
                                        .trim()
                                        .to_string()
                                } else if key_lower.starts_with("global ") {
                                    key_lower
                                        .strip_prefix("global ")
                                        .unwrap_or(&key_lower)
                                        .trim()
                                        .to_string()
                                } else {
                                    key_lower.trim().to_string()
                                };

                                if !seen_keys.contains(&dedupe_key) {
                                    seen_keys.insert(dedupe_key);
                                    deduped_items.push(item);
                                }
                            }
                        }
                        _ => deduped_items.push(item),
                    }
                }
                combined_sections.push(Section {
                    name,
                    items: deduped_items,
                });
            }
        }

        Ok(IniDocument {
            sections: combined_sections,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_in_logic_gate_and_wardrobe_gate() {
        let mut section = Section {
            name: "TextureOverrideBody".to_string(),
            items: vec![
                IniItem::Pair {
                    key: "hash".to_string(),
                    value: "deadbeef".to_string(),
                },
                IniItem::Pair {
                    key: "match_first_index".to_string(),
                    value: "0".to_string(),
                },
                IniItem::Pair {
                    key: "match_priority".to_string(),
                    value: "1".to_string(),
                },
                IniItem::Pair {
                    key: "vb0".to_string(),
                    value: "ResourceVB0".to_string(),
                },
                IniItem::Pair {
                    key: "ps-t0".to_string(),
                    value: "ResourceTexture".to_string(),
                },
                IniItem::Pair {
                    key: "drawindexed".to_string(),
                    value: "auto".to_string(),
                },
                IniItem::Command {
                    command: "run".to_string(),
                    args: vec!["CommandListTest".to_string()],
                },
            ],
        };

        Merger::wrap_in_logic_gate(&mut section, "$active == 1").unwrap();

        assert_eq!(section.items.len(), 9);
        assert_eq!(
            section.items[0],
            IniItem::Pair {
                key: "hash".to_string(),
                value: "deadbeef".to_string(),
            }
        );
        assert_eq!(
            section.items[1],
            IniItem::Pair {
                key: "match_first_index".to_string(),
                value: "0".to_string(),
            }
        );
        assert_eq!(
            section.items[2],
            IniItem::Pair {
                key: "match_priority".to_string(),
                value: "1".to_string(),
            }
        );
        assert_eq!(
            section.items[3],
            IniItem::Command {
                command: "if".to_string(),
                args: vec!["$active == 1".to_string()],
            }
        );
        assert_eq!(
            section.items[4],
            IniItem::Pair {
                key: "vb0".to_string(),
                value: "ResourceVB0".to_string(),
            }
        );
        assert_eq!(
            section.items[5],
            IniItem::Pair {
                key: "ps-t0".to_string(),
                value: "ResourceTexture".to_string(),
            }
        );
        assert_eq!(
            section.items[6],
            IniItem::Pair {
                key: "drawindexed".to_string(),
                value: "auto".to_string(),
            }
        );
        assert_eq!(
            section.items[7],
            IniItem::Command {
                command: "run".to_string(),
                args: vec!["CommandListTest".to_string()],
            }
        );
        assert_eq!(
            section.items[8],
            IniItem::Command {
                command: "endif".to_string(),
                args: vec![],
            }
        );

        // Test wrap_in_wardrobe_gate
        let mut wardrobe_section = Section {
            name: "TextureOverrideFace".to_string(),
            items: vec![
                IniItem::Pair {
                    key: "hash".to_string(),
                    value: "cafebabe".to_string(),
                },
                IniItem::Pair {
                    key: "match_first_index".to_string(),
                    value: "10".to_string(),
                },
                IniItem::Pair {
                    key: "match_priority".to_string(),
                    value: "2".to_string(),
                },
                IniItem::Pair {
                    key: "vb0".to_string(),
                    value: "ResourceFaceVB".to_string(),
                },
                IniItem::Pair {
                    key: "ps-t0".to_string(),
                    value: "ResourceFaceTex".to_string(),
                },
                IniItem::Pair {
                    key: "drawindexed".to_string(),
                    value: "auto".to_string(),
                },
                IniItem::Command {
                    command: "run".to_string(),
                    args: vec!["CommandListFace".to_string()],
                },
            ],
        };

        Merger::wrap_in_wardrobe_gate(&mut wardrobe_section, "wardrobe_keqing", 1).unwrap();

        assert_eq!(wardrobe_section.items.len(), 9);
        assert_eq!(
            wardrobe_section.items[0],
            IniItem::Pair {
                key: "hash".to_string(),
                value: "cafebabe".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[1],
            IniItem::Pair {
                key: "match_first_index".to_string(),
                value: "10".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[2],
            IniItem::Pair {
                key: "match_priority".to_string(),
                value: "2".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[3],
            IniItem::Command {
                command: "if".to_string(),
                args: vec!["$wardrobe_keqing == 1".to_string()],
            }
        );
        assert_eq!(
            wardrobe_section.items[4],
            IniItem::Pair {
                key: "vb0".to_string(),
                value: "ResourceFaceVB".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[5],
            IniItem::Pair {
                key: "ps-t0".to_string(),
                value: "ResourceFaceTex".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[6],
            IniItem::Pair {
                key: "drawindexed".to_string(),
                value: "auto".to_string(),
            }
        );
        assert_eq!(
            wardrobe_section.items[7],
            IniItem::Command {
                command: "run".to_string(),
                args: vec!["CommandListFace".to_string()],
            }
        );
        assert_eq!(
            wardrobe_section.items[8],
            IniItem::Command {
                command: "endif".to_string(),
                args: vec![],
            }
        );

        // Test section with no runtime body items remains unwrapped
        let mut header_only_section = Section {
            name: "TextureOverrideHair".to_string(),
            items: vec![
                IniItem::Pair {
                    key: "hash".to_string(),
                    value: "feedbeef".to_string(),
                },
                IniItem::Pair {
                    key: "match_first_index".to_string(),
                    value: "0".to_string(),
                },
                IniItem::Pair {
                    key: "match_priority".to_string(),
                    value: "0".to_string(),
                },
            ],
        };

        Merger::wrap_in_wardrobe_gate(&mut header_only_section, "wardrobe_keqing", 1).unwrap();
        assert_eq!(header_only_section.items.len(), 3);
        assert!(header_only_section
            .items
            .iter()
            .all(|item| !matches!(item, IniItem::Command { .. })));
    }
}
