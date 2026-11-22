use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogDiagnostic {
    pub level: String,    // "Error" | "Warning" | "Info"
    pub category: String, // "IniSyntax" | "DuplicateHash" | "MissingResource" | "PerfWarning" | "General"
    pub message: String,
    #[serde(default)]
    pub file_hint: Option<String>,
    #[serde(default)]
    pub line_number: Option<usize>,
}

/// Parses up to the tail `max_lines` of d3d11_log.txt, avoiding loading multi-GB
/// log files into memory, and extracts structured diagnostics.
pub fn parse_d3d11_log(log_path: &Path, max_lines: usize) -> Result<Vec<LogDiagnostic>> {
    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let mut file = File::open(log_path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    // Approximate average line length as ~256 bytes; seek to tail if file is large
    let approx_tail_bytes = (max_lines as u64) * 256;
    if file_len > approx_tail_bytes {
        let _ = file.seek(SeekFrom::Start(file_len - approx_tail_bytes));
    }

    let reader = BufReader::new(file);
    let mut all_lines: Vec<String> = Vec::new();
    for line in reader.lines().filter_map(|l| l.ok()) {
        all_lines.push(line);
    }

    // Keep up to max_lines from tail
    let start_idx = if all_lines.len() > max_lines {
        all_lines.len() - max_lines
    } else {
        0
    };

    let tail_slice = &all_lines[start_idx..];
    let mut diagnostics = Vec::new();

    for line in tail_slice {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_lowercase();

        // Filter only lines that convey problems, warnings, or notable status
        let is_error = lower.contains("error")
            || lower.contains("failed")
            || lower.contains("failure")
            || lower.contains("critical")
            || lower.contains("not found")
            || lower.contains("cannot find")
            || lower.contains("unable to");
        let is_warning = lower.contains("warning")
            || lower.contains("warn")
            || lower.contains("collision")
            || lower.contains("duplicate")
            || lower.contains("skipped")
            || lower.contains("missing");
        let is_info = lower.contains("loaded")
            || lower.contains("initialized")
            || lower.contains("installed");

        if !is_error && !is_warning && !is_info {
            continue;
        }

        let level = if is_error {
            "Error".to_string()
        } else if is_warning {
            "Warning".to_string()
        } else {
            "Info".to_string()
        };

        // Categorization
        let category = if lower.contains("syntax")
            || lower.contains("parse")
            || lower.contains("unclosed")
            || (lower.contains(".ini") && lower.contains("line"))
        {
            "IniSyntax".to_string()
        } else if lower.contains("duplicate hash")
            || lower.contains("hash collision")
            || (lower.contains("hash") && lower.contains("already registered"))
        {
            "DuplicateHash".to_string()
        } else if lower.contains("not found")
            || lower.contains("missing")
            || lower.contains("could not load")
            || lower.contains("failed to open")
            || lower.contains("cannot find")
        {
            "MissingResource".to_string()
        } else if lower.contains("perf")
            || lower.contains("performance")
            || lower.contains("slow")
            || lower.contains("fps drop")
        {
            "PerfWarning".to_string()
        } else {
            "General".to_string()
        };

        let file_hint = extract_file_hint(trimmed);
        let line_number = extract_line_number(trimmed);

        diagnostics.push(LogDiagnostic {
            level,
            category,
            message: trimmed.to_string(),
            file_hint,
            line_number,
        });
    }

    Ok(diagnostics)
}

fn extract_file_hint(text: &str) -> Option<String> {
    for word in text.split(|c: char| {
        c.is_whitespace() || c == '\'' || c == '"' || c == ',' || c == ':' || c == '(' || c == ')'
    }) {
        let w_low = word.to_lowercase();
        if w_low.ends_with(".ini")
            || w_low.ends_with(".dds")
            || w_low.ends_with(".buf")
            || w_low.ends_with(".ib")
            || w_low.ends_with(".hlsl")
            || (w_low.ends_with(".txt")
                && (w_low.contains("log") || w_low.contains("-vs=") || w_low.contains("-ps=")))
        {
            return Some(
                word.trim_matches(|c: char| {
                    !c.is_alphanumeric()
                        && c != '.'
                        && c != '_'
                        && c != '-'
                        && c != '/'
                        && c != '\\'
                })
                .to_string(),
            );
        }
    }
    None
}

fn extract_line_number(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find("line ") {
        let after = &lower[pos + 5..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num_str.parse::<usize>() {
            return Some(n);
        }
    }
    if let Some(pos) = lower.find("line:") {
        let after = &lower[pos + 5..];
        let num_str: String = after
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(n) = num_str.parse::<usize>() {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_d3d11_log_categories() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("d3d11_log.txt");

        let log_content = r#"
[INFO] Initializing 3DMigoto v1.3.16...
Parse error in Mods/KeqingMod/mod.ini at line 42: invalid command 'invalid_cmd'
WARNING: Duplicate hash deadbeef found in [TextureOverrideBody]
Resource file not found: Diffuse.dds for [ResourceDiffuse]
Performance warning: Draw call count exceeded 10000 in frame
Failed to open buffer: Head.buf
[INFO] Mod load sequence finished.
"#;
        std::fs::write(&log_path, log_content).unwrap();

        let diagnostics = parse_d3d11_log(&log_path, 100).unwrap();

        // 1. IniSyntax check
        let ini_diag = diagnostics
            .iter()
            .find(|d| d.category == "IniSyntax")
            .unwrap();
        assert_eq!(ini_diag.level, "Error");
        assert_eq!(ini_diag.line_number, Some(42));
        assert!(ini_diag.file_hint.as_deref().unwrap().contains("mod.ini"));

        // 2. DuplicateHash check
        let dup_diag = diagnostics
            .iter()
            .find(|d| d.category == "DuplicateHash")
            .unwrap();
        assert_eq!(dup_diag.level, "Warning");
        assert!(dup_diag.message.contains("deadbeef"));

        // 3. MissingResource check
        let missing_dds = diagnostics
            .iter()
            .find(|d| d.category == "MissingResource" && d.message.contains("Diffuse.dds"))
            .unwrap();
        assert_eq!(missing_dds.level, "Error");
        assert_eq!(missing_dds.file_hint.as_deref(), Some("Diffuse.dds"));

        let missing_buf = diagnostics
            .iter()
            .find(|d| d.category == "MissingResource" && d.message.contains("Head.buf"))
            .unwrap();
        assert_eq!(missing_buf.file_hint.as_deref(), Some("Head.buf"));

        // 4. PerfWarning check
        let perf_diag = diagnostics
            .iter()
            .find(|d| d.category == "PerfWarning")
            .unwrap();
        assert_eq!(perf_diag.level, "Warning");
    }
}
