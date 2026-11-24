use crate::error::ScriptError;
use ini::parser::parse_ini;
use ini::{IniCompiler, IniDocument, IniItem, IniPatcher};
use rhai::{Array, Dynamic, Engine, EvalAltResult, Scope};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptLimits {
    pub max_operations: u64,
    pub max_call_levels: usize,
    pub max_string_size: usize,
    pub max_array_size: usize,
    pub max_map_size: usize,
}

impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            max_operations: 500_000,
            max_call_levels: 32,
            max_string_size: 2 * 1024 * 1024, // 2 MiB
            max_array_size: 10_000,
            max_map_size: 5_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptAuditWarning {
    pub level: String, // "info" | "warning" | "blocker"
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ScriptExecutionResult {
    pub logs: Vec<String>,
    pub modified_files: Vec<String>,
    pub warnings: Vec<ScriptAuditWarning>,
    pub returned_json: serde_json::Value,
}

#[derive(Clone)]
struct HostState {
    root_dir: PathBuf,
    is_audit_mode: bool,
    dry_run: bool,
    logs: Arc<Mutex<Vec<String>>>,
    modified_files: Arc<Mutex<Vec<String>>>,
    warnings: Arc<Mutex<Vec<ScriptAuditWarning>>>,
    virtual_fs: Arc<Mutex<HashMap<String, String>>>,
}

impl HostState {
    fn resolve_safe_rel_path(&self, rel_path: &str) -> Result<PathBuf, ScriptError> {
        let norm = rel_path.replace('\\', "/");
        let trimmed = norm.trim();

        if trimmed.is_empty()
            || trimmed.starts_with('/')
            || trimmed.starts_with("./")
            || trimmed.split('/').any(|part| part == "..")
        {
            return Err(ScriptError::SecurityViolation(format!(
                "Path traversal rejected: {}",
                rel_path
            )));
        }

        let candidate = self.root_dir.join(trimmed);

        // Check if candidate resolves inside root_dir
        if let Ok(canon_root) = self.root_dir.canonicalize() {
            if candidate.exists() {
                if let Ok(canon_target) = candidate.canonicalize() {
                    if !canon_target.starts_with(&canon_root) {
                        return Err(ScriptError::SecurityViolation(format!(
                            "Symlink or path escaped root: {}",
                            rel_path
                        )));
                    }
                }
            }
        }

        Ok(candidate)
    }
}

pub struct ScriptHost;

impl ScriptHost {
    fn build_engine(state: HostState, limits: ScriptLimits) -> Engine {
        let mut engine = Engine::new();

        // 1. Enforce Sandbox Limits
        engine.set_max_operations(limits.max_operations);
        engine.set_max_call_levels(limits.max_call_levels);
        engine.set_max_string_size(limits.max_string_size);
        engine.set_max_array_size(limits.max_array_size);
        engine.set_max_map_size(limits.max_map_size);

        // 2. Register Logging & Audit APIs
        {
            let logs = state.logs.clone();
            engine.register_fn("log_info", move |msg: &str| {
                if let Ok(mut l) = logs.lock() {
                    l.push(format!("[INFO] {}", msg));
                }
            });
        }
        {
            let logs = state.logs.clone();
            engine.register_fn("log_warn", move |msg: &str| {
                if let Ok(mut l) = logs.lock() {
                    l.push(format!("[WARN] {}", msg));
                }
            });
        }
        {
            let logs = state.logs.clone();
            engine.register_fn("log_error", move |msg: &str| {
                if let Ok(mut l) = logs.lock() {
                    l.push(format!("[ERROR] {}", msg));
                }
            });
        }
        {
            let warnings = state.warnings.clone();
            engine.register_fn(
                "emit_warning",
                move |level: &str, title: &str, message: &str| {
                    if let Ok(mut w) = warnings.lock() {
                        w.push(ScriptAuditWarning {
                            level: level.to_string(),
                            title: title.to_string(),
                            message: message.to_string(),
                        });
                    }
                },
            );
        }

        // 3. Register Jailed mod_fs APIs
        {
            let s = state.clone();
            engine.register_fn("fs_list_files", move || -> Array {
                let mut list = Array::new();
                for entry in WalkDir::new(&s.root_dir).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        if let Ok(rel) = path.strip_prefix(&s.root_dir) {
                            let rel_str = rel.to_string_lossy().replace('\\', "/");
                            if !rel_str.starts_with(".yago_backups") {
                                list.push(Dynamic::from(rel_str));
                            }
                        }
                    }
                }
                list
            });
        }
        {
            let s = state.clone();
            engine.register_fn(
                "fs_read_text",
                move |rel_path: &str| -> Result<String, Box<EvalAltResult>> {
                    let target_path = s
                        .resolve_safe_rel_path(rel_path)
                        .map_err(|e| e.to_string())?;

                    let norm_rel = rel_path.replace('\\', "/");
                    if let Ok(vfs) = s.virtual_fs.lock() {
                        if let Some(content) = vfs.get(&norm_rel) {
                            return Ok(content.clone());
                        }
                    }

                    if !target_path.exists() {
                        return Err(format!("File not found: {}", rel_path).into());
                    }

                    std::fs::read_to_string(&target_path).map_err(|e| e.to_string().into())
                },
            );
        }
        {
            let s = state.clone();
            engine.register_fn(
                "fs_write_text",
                move |rel_path: &str, content: &str| -> Result<(), Box<EvalAltResult>> {
                    if s.is_audit_mode {
                        return Err("Security Violation: fs_write_text is not permitted in audit mode (read-only)".into());
                    }

                    let target_path = s
                        .resolve_safe_rel_path(rel_path)
                        .map_err(|e| e.to_string())?;

                    let norm_rel = rel_path.replace('\\', "/");

                    if s.dry_run {
                        if let Ok(mut vfs) = s.virtual_fs.lock() {
                            vfs.insert(norm_rel.clone(), content.to_string());
                        }
                    } else {
                        if let Some(parent) = target_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        std::fs::write(&target_path, content).map_err(|e| e.to_string())?;
                    }

                    if let Ok(mut modified) = s.modified_files.lock() {
                        if !modified.contains(&norm_rel) {
                            modified.push(norm_rel);
                        }
                    }

                    Ok(())
                },
            );
        }
        {
            let s = state.clone();
            engine.register_fn(
                "fs_exists",
                move |rel_path: &str| -> Result<bool, Box<EvalAltResult>> {
                    let target_path = s
                        .resolve_safe_rel_path(rel_path)
                        .map_err(|e| e.to_string())?;
                    let norm_rel = rel_path.replace('\\', "/");
                    if let Ok(vfs) = s.virtual_fs.lock() {
                        if vfs.contains_key(&norm_rel) {
                            return Ok(true);
                        }
                    }
                    Ok(target_path.exists())
                },
            );
        }

        // 4. Register Scoped INI APIs
        engine.register_fn(
            "ini_replace_hash",
            |content: &str, old_hash: &str, new_hash: &str| -> String {
                let mut out = Vec::new();
                let old_lower = old_hash.trim().to_lowercase();
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.to_lowercase().starts_with("hash") && trimmed.contains('=') {
                        let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                        if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case("hash") {
                            let val = parts[1].trim();
                            if val.to_lowercase() == old_lower {
                                out.push(format!("hash = {}", new_hash));
                                continue;
                            }
                        }
                    }
                    out.push(line.to_string());
                }
                out.join("\n")
            },
        );

        engine.register_fn(
            "ini_has_section",
            |content: &str, section_name: &str| -> bool {
                if let Ok((_, doc)) = parse_ini(content) {
                    doc.sections
                        .iter()
                        .any(|s| s.name.eq_ignore_ascii_case(section_name))
                } else {
                    false
                }
            },
        );

        engine.register_fn(
            "ini_get_value",
            |content: &str, section_name: &str, key: &str| -> String {
                if let Ok((_, doc)) = parse_ini(content) {
                    for sec in &doc.sections {
                        if sec.name.eq_ignore_ascii_case(section_name) {
                            for item in &sec.items {
                                if let IniItem::Pair { key: k, value: v } = item {
                                    if k.eq_ignore_ascii_case(key) {
                                        return v.clone();
                                    }
                                }
                            }
                        }
                    }
                }
                String::new()
            },
        );

        engine.register_fn(
            "ini_set_value",
            |content: &str, section_name: &str, key: &str, value: &str| -> String {
                let mut doc = if let Ok((_, d)) = parse_ini(content) {
                    d
                } else {
                    IniDocument::default()
                };
                doc.set_value(section_name, key, value);
                IniCompiler::default().serialize(&doc)
            },
        );

        engine
    }

    /// Compiles and runs a mod fixer script against the given mod root.
    pub fn run_fixer_script(
        script_src: &str,
        mod_root: &Path,
        dry_run: bool,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        let state = HostState {
            root_dir: mod_root.to_path_buf(),
            is_audit_mode: false,
            dry_run,
            logs: Arc::new(Mutex::new(Vec::new())),
            modified_files: Arc::new(Mutex::new(Vec::new())),
            warnings: Arc::new(Mutex::new(Vec::new())),
            virtual_fs: Arc::new(Mutex::new(HashMap::new())),
        };

        let engine = Self::build_engine(state.clone(), ScriptLimits::default());
        let ast = engine
            .compile(script_src)
            .map_err(|e| ScriptError::Compile(e.to_string()))?;

        let mut scope = Scope::new();
        let eval_res = engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);

        let returned_json = match eval_res {
            Ok(dyn_val) => rhai::serde::from_dynamic::<serde_json::Value>(&dyn_val)
                .unwrap_or(serde_json::Value::Null),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Security Violation") {
                    return Err(ScriptError::SecurityViolation(err_str));
                }
                return Err(ScriptError::Runtime(err_str));
            }
        };

        let logs = state.logs.lock().map(|l| l.clone()).unwrap_or_default();
        let modified_files = state
            .modified_files
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        let warnings = state.warnings.lock().map(|w| w.clone()).unwrap_or_default();

        Ok(ScriptExecutionResult {
            logs,
            modified_files,
            warnings,
            returned_json,
        })
    }

    /// Compiles and runs a read-only audit script against the given game directory.
    pub fn run_audit_script(
        script_src: &str,
        game_dir: &Path,
    ) -> Result<ScriptExecutionResult, ScriptError> {
        let state = HostState {
            root_dir: game_dir.to_path_buf(),
            is_audit_mode: true,
            dry_run: true,
            logs: Arc::new(Mutex::new(Vec::new())),
            modified_files: Arc::new(Mutex::new(Vec::new())),
            warnings: Arc::new(Mutex::new(Vec::new())),
            virtual_fs: Arc::new(Mutex::new(HashMap::new())),
        };

        let engine = Self::build_engine(state.clone(), ScriptLimits::default());
        let ast = engine
            .compile(script_src)
            .map_err(|e| ScriptError::Compile(e.to_string()))?;

        let mut scope = Scope::new();
        let eval_res = engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);

        let returned_json = match eval_res {
            Ok(dyn_val) => rhai::serde::from_dynamic::<serde_json::Value>(&dyn_val)
                .unwrap_or(serde_json::Value::Null),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Security Violation") {
                    return Err(ScriptError::SecurityViolation(err_str));
                }
                return Err(ScriptError::Runtime(err_str));
            }
        };

        let logs = state.logs.lock().map(|l| l.clone()).unwrap_or_default();
        let modified_files = state
            .modified_files
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        let warnings = state.warnings.lock().map(|w| w.clone()).unwrap_or_default();

        Ok(ScriptExecutionResult {
            logs,
            modified_files,
            warnings,
            returned_json,
        })
    }
}
