use crate::error::{Result, WeaverError};
use ini::ast::{IniDocument, IniItem};
use std::collections::{HashMap, HashSet};

pub struct Validator;

impl Validator {
    /// Validates the logic syntax of an INI document.
    pub fn validate_logic(doc: &IniDocument) -> Result<()> {
        // 1. Balanced If/Endif Check
        for section in &doc.sections {
            let mut depth = 0;
            for item in &section.items {
                if let IniItem::Command { command, .. } = item {
                    let cmd = command.to_lowercase();
                    if cmd == "if" || cmd.starts_with("if_") {
                        depth += 1;
                    } else if cmd == "endif" {
                        if depth == 0 {
                            return Err(WeaverError::ValidationError(format!(
                                "Unexpected 'endif' in section [{}] (no matching if)",
                                section.name
                            )));
                        }
                        depth -= 1;
                    } else if (cmd == "else" || cmd == "elif" || cmd == "else if") && depth == 0 {
                        return Err(WeaverError::ValidationError(format!(
                            "Unexpected '{}' in section [{}] (outside of if block)",
                            cmd, section.name
                        )));
                    }
                }
            }
            if depth > 0 {
                return Err(WeaverError::ValidationError(format!(
                    "Unclosed 'if' block in section [{}] (missing {} endifs)",
                    section.name, depth
                )));
            }
        }

        // 2. Deep Cycle Detection
        Self::detect_logic_cycles(doc)?;

        // 3. Ghost Variable & Duplicate Global Audit
        Self::audit_variables(doc)?;

        Ok(())
    }

    /// Detects deep circular recursion in CommandLists (A -> B -> A).
    pub fn detect_logic_cycles(doc: &IniDocument) -> Result<()> {
        let mut adj = HashMap::new();
        for section in &doc.sections {
            let name = section.name.to_lowercase();
            if name.starts_with("commandlist") {
                let mut targets = Vec::new();
                for item in &section.items {
                    if let IniItem::Command { command, args } = item {
                        let cmd = command.to_lowercase();
                        if cmd == "run" || cmd == "pre run" || cmd == "post run" {
                            for arg in args {
                                targets.push(arg.to_lowercase());
                            }
                        }
                    }
                }
                adj.insert(name, targets);
            }
        }

        let mut visited = HashSet::new();
        let mut stack = HashSet::new();

        for node in adj.keys() {
            if Self::has_cycle(node, &adj, &mut visited, &mut stack) {
                return Err(WeaverError::ValidationError(format!(
                    "Infinite logic loop detected in CommandList: '{}'",
                    node
                )));
            }
        }
        Ok(())
    }

    fn has_cycle(
        node: &String,
        adj: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
    ) -> bool {
        if stack.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }

        visited.insert(node.clone());
        stack.insert(node.clone());

        if let Some(neighbors) = adj.get(node) {
            for neighbor in neighbors {
                if adj.contains_key(neighbor) && Self::has_cycle(neighbor, adj, visited, stack) {
                    return true;
                }
            }
        }

        stack.remove(node);
        false
    }

    /// Detects usage of variables that were never declared and duplicate declarations.
    pub fn audit_variables(doc: &IniDocument) -> Result<()> {
        let mut defined_globals = HashSet::new();
        let mut section_locals: HashMap<String, HashSet<String>> = HashMap::new();

        // Pass 1: Collect all global declarations and check for duplicates, and local declarations scoped to section
        for section in &doc.sections {
            let sec_key = section.name.to_lowercase();
            for item in &section.items {
                if let IniItem::Pair { key, .. } = item {
                    let k = key.to_lowercase();
                    if k.starts_with("global") {
                        for word in k.split_whitespace() {
                            if word.starts_with('$') {
                                let var = word
                                    .trim_matches(|c: char| {
                                        !c.is_alphanumeric()
                                            && c != '$'
                                            && c != '_'
                                            && c != '\\'
                                            && c != '.'
                                    })
                                    .to_lowercase();
                                if var.starts_with('$') {
                                    if !defined_globals.insert(var.clone()) {
                                        return Err(WeaverError::ValidationError(format!(
                                            "Duplicate global variable declaration: '{}'",
                                            var
                                        )));
                                    }
                                    break;
                                }
                            }
                        }
                    } else if k.starts_with("local") {
                        for word in k.split_whitespace() {
                            if word.starts_with('$') {
                                let var = word
                                    .trim_matches(|c: char| {
                                        !c.is_alphanumeric()
                                            && c != '$'
                                            && c != '_'
                                            && c != '\\'
                                            && c != '.'
                                    })
                                    .to_lowercase();
                                if var.starts_with('$') {
                                    section_locals
                                        .entry(sec_key.clone())
                                        .or_default()
                                        .insert(var);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let empty_set = HashSet::new();

        // Pass 2: Check usage in commands and values
        for section in &doc.sections {
            let sec_key = section.name.to_lowercase();
            let locals = section_locals.get(&sec_key).unwrap_or(&empty_set);

            let check_text = |text: &str, sec_name: &str| -> Result<()> {
                for part in text.split(|c: char| {
                    !c.is_alphanumeric() && c != '$' && c != '_' && c != '\\' && c != '.'
                }) {
                    if part.starts_with('$') {
                        if part.starts_with("$\\") {
                            continue;
                        }
                        let clean_part = part.trim_end_matches('.');
                        let var = clean_part.to_lowercase();
                        let base_var = var.split('.').next().unwrap_or(&var);
                        if !defined_globals.contains(&var)
                            && !locals.contains(&var)
                            && !defined_globals.contains(base_var)
                            && !locals.contains(base_var)
                        {
                            return Err(WeaverError::ValidationError(format!(
                                "Ghost variable detected: '{}' in section [{}]",
                                clean_part, sec_name
                            )));
                        }
                    }
                }
                Ok(())
            };

            for item in &section.items {
                match item {
                    IniItem::Command { command, args } => {
                        let cmd = command.to_lowercase();
                        if cmd == "run" || cmd == "pre run" || cmd == "post run" {
                            continue;
                        } // Already handled by cycle detection

                        for arg in args {
                            check_text(arg, &section.name)?;
                        }
                    }
                    IniItem::Pair { key, value } => {
                        let k = key.to_lowercase();
                        if !k.starts_with("global") && !k.starts_with("local") {
                            check_text(key, &section.name)?;
                        }
                        check_text(value, &section.name)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ini::parser::parse_ini;

    #[test]
    fn test_valid_if_elif_else_if_else_endif() {
        let input = r#"
[Constants]
global $a = 1
global $b = 2
global $c = 3

[CommandListTest]
if $a == 1
    x = 1
elif $b == 2
    x = 2
else if $c == 3
    x = 3
else
    x = 4
endif
"#;
        let (_, doc) = parse_ini(input).unwrap();
        assert!(Validator::validate_logic(&doc).is_ok());
    }

    #[test]
    fn test_rejection_of_orphan_elif() {
        let input = r#"
[CommandListTest]
elif $b == 2
    x = 2
"#;
        let (_, doc) = parse_ini(input).unwrap();
        let result = Validator::validate_logic(&doc);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Unexpected 'elif' in section [CommandListTest] (outside of if block)")
        );

        let input_else_if = r#"
[CommandListTest]
else if $b == 2
    x = 2
"#;
        let (_, doc_else_if) = parse_ini(input_else_if).unwrap();
        let result_else_if = Validator::validate_logic(&doc_else_if);
        assert!(result_else_if.is_err());
        let err_else_if = result_else_if.unwrap_err().to_string();
        assert!(err_else_if
            .contains("Unexpected 'else if' in section [CommandListTest] (outside of if block)"));
    }

    #[test]
    fn test_acceptance_of_cross_namespace_variables() {
        let input = r#"
[CommandListTest]
x = $\TexFx\transparency
if $\global\tracking\swimming == 1
    y = 1
endif
"#;
        let (_, doc) = parse_ini(input).unwrap();
        assert!(Validator::validate_logic(&doc).is_ok());
    }

    #[test]
    fn test_acceptance_of_section_scoped_local_variables() {
        let input = r#"
[CommandList1]
local $tmp = 1
x = $tmp

[CommandList2]
local $tmp = 2
y = $tmp
"#;
        let (_, doc) = parse_ini(input).unwrap();
        assert!(Validator::validate_logic(&doc).is_ok());

        // Local variable from CommandList1 should not be accessible in CommandList3
        let invalid_input = r#"
[CommandList1]
local $tmp = 1

[CommandList3]
z = $tmp
"#;
        let (_, doc_inv) = parse_ini(invalid_input).unwrap();
        let result = Validator::validate_logic(&doc_inv);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ghost variable detected: '$tmp' in section [CommandList3]"));
    }

    #[test]
    fn test_continued_rejection_of_undeclared_typo_var() {
        let input = r#"
[Constants]
global $valid = 1

[CommandListTest]
x = $typo_var
"#;
        let (_, doc) = parse_ini(input).unwrap();
        let result = Validator::validate_logic(&doc);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ghost variable detected: '$typo_var' in section [CommandListTest]"));
    }

    #[test]
    fn test_cycle_detection_across_pre_post_run() {
        let input = r#"
[CommandListA]
pre run = CommandListB

[CommandListB]
post run = CommandListA
"#;
        let (_, doc) = parse_ini(input).unwrap();
        let result = Validator::detect_logic_cycles(&doc);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Infinite logic loop detected in CommandList: 'commandlista'")
                || err.contains("Infinite logic loop detected in CommandList: 'commandlistb'")
        );
    }
}
