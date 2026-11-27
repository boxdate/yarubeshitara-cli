use crate::{
    expand_tilde, CoachJaConfig, BUILTIN_DIALECT_HOKKAIDO_DONAN, BUILTIN_DIALECT_KANSAI,
    BUILTIN_DIALECT_STANDARD_POLITE,
};
use candle_plamo3::pfml3::DialectConfig;
use std::path::{Path, PathBuf};

pub struct BuiltinDialectPreset {
    pub number: usize,
    pub id: &'static str,
    pub filename: &'static str,
    pub json_str: &'static str,
}

pub const BUILTIN_DIALECT_PRESETS: &[BuiltinDialectPreset] = &[
    BuiltinDialectPreset {
        number: 1,
        id: "standard_polite",
        filename: "standard_polite.json",
        json_str: BUILTIN_DIALECT_STANDARD_POLITE,
    },
    BuiltinDialectPreset {
        number: 2,
        id: "hokkaido_donan",
        filename: "hokkaido_donan.json",
        json_str: BUILTIN_DIALECT_HOKKAIDO_DONAN,
    },
    BuiltinDialectPreset {
        number: 3,
        id: "kansai",
        filename: "kansai.json",
        json_str: BUILTIN_DIALECT_KANSAI,
    },
];

#[derive(Debug, Clone)]
pub struct ResolvedDialect {
    pub id: String,
    pub config: DialectConfig,
    pub source_path: Option<PathBuf>,
    pub is_builtin_fallback: bool,
}

pub fn normalize_preset_id(raw: &str) -> Option<&'static BuiltinDialectPreset> {
    let lower = raw
        .trim()
        .trim_end_matches(".json")
        .to_lowercase()
        .replace('-', "_");
    match lower.as_str() {
        "standard_polite" | "standard" | "polite" | "default" | "標準語" => {
            Some(&BUILTIN_DIALECT_PRESETS[0])
        }
        "hokkaido_donan" | "hokkaido" | "donan" | "yarube" | "北海道" | "道南弁" => {
            Some(&BUILTIN_DIALECT_PRESETS[1])
        }
        "kansai" | "osaka" | "関西弁" => Some(&BUILTIN_DIALECT_PRESETS[2]),
        _ => None,
    }
}

pub fn load_builtin_dialect(preset: &BuiltinDialectPreset) -> DialectConfig {
    serde_json::from_str::<DialectConfig>(preset.json_str)
        .unwrap_or_else(|_| DialectConfig::standard_polite())
}

/// Resolves the requested dialect (preset name, filename in `dialects_dir`, or direct file path)
/// into a `ResolvedDialect`.
pub fn resolve_dialect_config(cfg: &CoachJaConfig) -> ResolvedDialect {
    resolve_dialect_by_spec(&cfg.dialect, &cfg.dialects_dir)
}

pub fn resolve_dialect_by_spec(spec: &str, dialects_dir_str: &str) -> ResolvedDialect {
    let trimmed = spec.trim();
    let dialects_dir = expand_tilde(dialects_dir_str);

    // 1. Direct path if `spec` points to an existing JSON file
    let expanded_spec = expand_tilde(trimmed);
    if expanded_spec.is_file() {
        if let Ok(config) = DialectConfig::from_json_file(&expanded_spec) {
            let id = expanded_spec
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(trimmed)
                .to_string();
            return ResolvedDialect {
                id,
                config,
                source_path: Some(expanded_spec),
                is_builtin_fallback: false,
            };
        }
    }

    // 2. Check inside `dialects_dir` (`<spec>.json` or `<spec>`)
    let stem = trimmed.trim_end_matches(".json");
    let candidate_in_dir = dialects_dir.join(format!("{stem}.json"));
    if candidate_in_dir.is_file() {
        if let Ok(config) = DialectConfig::from_json_file(&candidate_in_dir) {
            return ResolvedDialect {
                id: stem.to_string(),
                config,
                source_path: Some(candidate_in_dir),
                is_builtin_fallback: false,
            };
        }
    }

    // 3. Check normalized preset ID in `dialects_dir` or built-in preset JSON
    if let Some(preset) = normalize_preset_id(trimmed) {
        let preset_in_dir = dialects_dir.join(preset.filename);
        if preset_in_dir.is_file() {
            if let Ok(config) = DialectConfig::from_json_file(&preset_in_dir) {
                return ResolvedDialect {
                    id: preset.id.to_string(),
                    config,
                    source_path: Some(preset_in_dir),
                    is_builtin_fallback: false,
                };
            }
        }
        return ResolvedDialect {
            id: preset.id.to_string(),
            config: load_builtin_dialect(preset),
            source_path: None,
            is_builtin_fallback: true,
        };
    }

    // 4. Fallback to default `hokkaido_donan` built-in preset
    let default_preset = &BUILTIN_DIALECT_PRESETS[1];
    ResolvedDialect {
        id: default_preset.id.to_string(),
        config: load_builtin_dialect(default_preset),
        source_path: None,
        is_builtin_fallback: true,
    }
}

/// Lists all available dialects (built-in presets + any extra `*.json` files in `dialects_dir`).
pub fn list_available_dialects(dialects_dir_str: &str) -> Vec<ResolvedDialect> {
    let mut results = Vec::new();
    let dialects_dir = expand_tilde(dialects_dir_str);

    for preset in BUILTIN_DIALECT_PRESETS {
        results.push(resolve_dialect_by_spec(preset.id, dialects_dir_str));
    }

    if let Ok(entries) = std::fs::read_dir(&dialects_dir) {
        let mut custom_paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        custom_paths.sort();

        for path in custom_paths {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if BUILTIN_DIALECT_PRESETS.iter().any(|p| p.id == stem) {
                continue;
            }
            if let Ok(config) = DialectConfig::from_json_file(&path) {
                results.push(ResolvedDialect {
                    id: stem.to_string(),
                    config,
                    source_path: Some(path),
                    is_builtin_fallback: false,
                });
            }
        }
    }

    results
}

/// Writes the built-in dialect JSON files to `dialects_dir` if they do not exist yet.
/// Returns the paths of newly created files.
pub fn ensure_default_dialect_files(dialects_dir: &Path) -> Vec<PathBuf> {
    let mut created = Vec::new();
    let _ = std::fs::create_dir_all(dialects_dir);
    for preset in BUILTIN_DIALECT_PRESETS {
        let path = dialects_dir.join(preset.filename);
        if !path.exists() && std::fs::write(&path, preset.json_str).is_ok() {
            created.push(path);
        }
    }
    created
}

/// Updates or inserts `dialect = "<new_dialect>"` under `[plugins.coach-ja]` in `config.toml`
/// (and syncs to any other existing yarube config file such as `~/.yarube/config` or `~/.config/yarube/config.toml`)
/// while preserving all other sections and comments.
pub fn update_config_dialect(config_path: &Path, new_dialect: &str) -> std::io::Result<()> {
    if let Some(parent) = config_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let updated = upsert_coach_ja_dialect_in_toml(&existing, new_dialect);
    std::fs::write(config_path, updated)?;

    // Also sync to other existing config files if YARUBE_CONFIG_PATH wasn't explicitly overridden in a test
    if std::env::var("YARUBE_CONFIG_PATH").is_err() {
        for extra in [
            "~/.config/yarube/config.toml",
            "~/.yarube/config.toml",
            "~/.yarube/config",
        ] {
            let p = expand_tilde(extra);
            if p != config_path && p.is_file() {
                if let Ok(ext_content) = std::fs::read_to_string(&p) {
                    let ext_updated = upsert_coach_ja_dialect_in_toml(&ext_content, new_dialect);
                    let _ = std::fs::write(&p, ext_updated);
                }
            }
        }
    }

    Ok(())
}

pub fn upsert_coach_ja_dialect_in_toml(content: &str, new_dialect: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut section_start: Option<usize> = None;
    let mut section_end: usize = lines.len();
    let mut dialect_line_idx: Option<usize> = None;

    let mut in_target_section = false;
    for (idx, raw_line) in lines.iter().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = trimmed[1..trimmed.len() - 1].trim();
            if sec_name == "plugins.coach-ja" || sec_name == "plugins.coach_ja" {
                in_target_section = true;
                section_start = Some(idx);
            } else if in_target_section {
                section_end = idx;
                break;
            }
            continue;
        }
        if in_target_section && !trimmed.starts_with('#') {
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim() == "dialect" {
                    dialect_line_idx = Some(idx);
                }
            }
        }
    }

    let formatted_line = format!("dialect = \"{}\"", new_dialect);

    if let Some(d_idx) = dialect_line_idx {
        lines[d_idx] = formatted_line;
    } else if let Some(s_start) = section_start {
        let insert_pos = (s_start + 1).min(section_end);
        lines.insert(insert_pos, formatted_line);
    } else {
        if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
            lines.push(String::new());
        }
        lines.push("[plugins.coach-ja]".to_string());
        lines.push(formatted_line);
    }

    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}
