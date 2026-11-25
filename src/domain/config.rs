use crate::domain::i18n::Language;
use crate::domain::theme::{ThemeName, ThemeResolution};
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

pub const DEFAULT_PLUGINS_DIR: &str = "~/.local/share/yarube/plugins";

pub fn default_plugins_dir() -> String {
    DEFAULT_PLUGINS_DIR.to_string()
}

pub fn default_on_reflect() -> Vec<String> {
    vec!["coach".to_string()]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginsConfig {
    pub dir: String,
    pub on_reflect: Vec<String>,
    pub on_task_update: Vec<String>,
    pub plugin_enabled: BTreeMap<String, bool>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            dir: default_plugins_dir(),
            on_reflect: default_on_reflect(),
            on_task_update: Vec::new(),
            plugin_enabled: BTreeMap::new(),
        }
    }
}

impl PluginsConfig {
    /// Expands a leading `~` or `~/` to the user's home directory (`$HOME` or `$USERPROFILE`) and resolves to an absolute path.
    pub fn resolved_dir(&self) -> PathBuf {
        resolve_abs_path(&self.dir)
    }

    /// Checks whether a given plugin `<name>` is enabled (defaults to `true` unless `[plugins.<name>] enabled = false`).
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugin_enabled.get(name).copied().unwrap_or(true)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Expands a leading `~` or `~/` using `$HOME` (or `$USERPROFILE` on Windows).
pub fn expand_tilde(path_str: &str) -> PathBuf {
    let trimmed = path_str.trim();
    if trimmed == "~" || trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        if let Some(home) = home_dir() {
            if trimmed == "~" {
                return home;
            }
            let rest = &trimmed[2..];
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

/// Expands `~` and ensures the resulting `PathBuf` is an absolute path independent of relative working directory assumptions.
pub fn resolve_abs_path(path_str: &str) -> PathBuf {
    let expanded = expand_tilde(path_str);
    ensure_abs_path(expanded)
}

pub fn ensure_abs_path(path: PathBuf) -> PathBuf {
    if let Some(s) = path.to_str() {
        if s == "~" || s.starts_with("~/") || s.starts_with("~\\") {
            let expanded = expand_tilde(s);
            if expanded.is_absolute() {
                return std::fs::canonicalize(&expanded).unwrap_or(expanded);
            }
        }
    }
    if path.is_absolute() {
        std::fs::canonicalize(&path).unwrap_or(path)
    } else if let Ok(canon) = std::fs::canonicalize(&path) {
        canon
    } else if let Ok(cwd) = env::current_dir() {
        cwd.join(path)
    } else if let Some(home) = home_dir() {
        home.join(path)
    } else {
        path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub editor: Option<String>,
    pub lang: Option<String>,
    pub theme: Option<String>,
    pub plugins: PluginsConfig,
}

pub struct EditorPreset {
    pub number: usize,
    pub command: &'static str,
    pub label_ja: &'static str,
    pub label_en: &'static str,
}

impl EditorPreset {
    pub fn label(&self, lang: Language) -> &'static str {
        match lang {
            Language::Ja => self.label_ja,
            Language::En => self.label_en,
        }
    }
}

pub const EDITOR_PRESETS: &[EditorPreset] = &[
    EditorPreset {
        number: 1,
        command: "nano",
        label_ja: "nano (シンプル・標準CUIエディタ)",
        label_en: "nano (Simple standard terminal editor)",
    },
    EditorPreset {
        number: 2,
        command: "vim",
        label_ja: "vim (高機能CUIエディタ)",
        label_en: "vim (Vi Improved terminal editor)",
    },
    EditorPreset {
        number: 3,
        command: "nvim",
        label_ja: "nvim (Neovim)",
        label_en: "nvim (Neovim)",
    },
    EditorPreset {
        number: 4,
        command: "code --wait",
        label_ja: "code --wait (VS Code / 保存終了まで待機)",
        label_en: "code --wait (VS Code / wait until tab closed)",
    },
];

impl AppConfig {
    pub fn parse(content: &str) -> Self {
        let mut editor = None;
        let mut lang = None;
        let mut theme = None;
        let mut plugins = PluginsConfig::default();
        let mut current_section = "";

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                let inner = line[1..line.len() - 1].trim();
                if let Some((sec_part, _comment)) = inner.split_once(']') {
                    current_section = sec_part.trim();
                } else {
                    current_section = inner;
                }
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let k = key.trim();
                let raw_val = strip_toml_comment(val).trim();

                if current_section == "plugins" {
                    match k {
                        "dir" => {
                            let v = unquote_str(raw_val).trim();
                            plugins.dir = if v.is_empty() {
                                default_plugins_dir()
                            } else {
                                v.to_string()
                            };
                        }
                        "on_reflect" => {
                            if let Some(arr) = parse_toml_string_array(raw_val) {
                                plugins.on_reflect = arr;
                            }
                        }
                        "on_task_update" => {
                            if let Some(arr) = parse_toml_string_array(raw_val) {
                                plugins.on_task_update = arr;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                if let Some(plugin_sub) = current_section.strip_prefix("plugins.") {
                    let plugin_name = plugin_sub.trim();
                    if !plugin_name.is_empty() && k == "enabled" {
                        let v_lower = unquote_str(raw_val).to_lowercase();
                        if v_lower == "false" {
                            plugins.plugin_enabled.insert(plugin_name.to_string(), false);
                        } else if v_lower == "true" {
                            plugins.plugin_enabled.insert(plugin_name.to_string(), true);
                        }
                    }
                    continue;
                }

                let v = unquote_str(raw_val);
                if v.is_empty() {
                    continue;
                }
                match (current_section, k) {
                    ("" | "general", "editor") => editor = Some(v.to_string()),
                    ("" | "general", "lang" | "language") => lang = Some(v.to_string()),
                    ("" | "general", "theme") | ("theme", "name") => theme = Some(v.to_string()),
                    _ => {}
                }
            }
        }
        Self {
            editor,
            lang,
            theme,
            plugins,
        }
    }

    pub fn to_config_string(&self) -> String {
        let mut out = String::from("# Yarubeshitara! (yarube) 設定ファイル\n");
        if let Some(ref ed) = self.editor {
            out.push_str(&format!("editor = \"{}\"\n", ed));
        }
        if let Some(ref lg) = self.lang {
            out.push_str(&format!("lang = \"{}\"\n", lg));
        }
        if let Some(ref th) = self.theme {
            out.push_str(&format!("theme = \"{}\"\n", th));
        }
        out.push_str("\n[plugins]\n");
        out.push_str(&format!("dir = \"{}\"\n", self.plugins.dir));
        out.push_str(&format!(
            "on_reflect = {}\n",
            format_toml_string_array(&self.plugins.on_reflect)
        ));
        out.push_str(&format!(
            "on_task_update = {}\n",
            format_toml_string_array(&self.plugins.on_task_update)
        ));
        for (p_name, enabled) in &self.plugins.plugin_enabled {
            out.push_str(&format!("\n[plugins.{}]\nenabled = {}\n", p_name, enabled));
        }
        out
    }

    /// Resolves the active display language (`config` > `YARUBE_LANG`/`BUJO_LANG` > `LC_ALL` > `LC_MESSAGES` > `LANG` > `Ja`).
    pub fn resolve_language(&self) -> Language {
        Language::resolve(self.lang.as_deref())
    }

    /// Resolves the color theme (`config` > `YARUBE_THEME`/`BUJO_THEME` > default `tokyo-night`) along with any unknown theme warning (EARS-18, EARS-21).
    pub fn resolve_theme(&self) -> ThemeResolution {
        if let Some(ref cfg_theme) = self.theme {
            return ThemeName::resolve(Some(cfg_theme));
        }
        for key in ["YARUBE_THEME", "BUJO_THEME"] {
            if let Ok(env_theme) = env::var(key) {
                if !env_theme.trim().is_empty() {
                    return ThemeName::resolve(Some(&env_theme));
                }
            }
        }
        ThemeName::resolve(None)
    }

    /// Resolves the editor command according to priority (TP-12 / EARS-12):
    /// 1. Explicit config file (`self.editor`)
    /// 2. Environment variable `$VISUAL`
    /// 3. Environment variable `$EDITOR`
    /// 4. System default (`notepad` on Windows, `nano` on Unix/Linux/macOS)
    pub fn resolve_editor(&self) -> String {
        self.resolve_editor_with(
            |key| env::var(key).ok(),
            if cfg!(target_os = "windows") {
                "notepad"
            } else {
                "nano"
            },
        )
    }

    pub fn resolve_editor_with<F>(&self, get_env: F, system_default: &str) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(ref cfg_ed) = self.editor {
            let trimmed = cfg_ed.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(vis) = get_env("VISUAL") {
            let trimmed = vis.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        if let Some(ed) = get_env("EDITOR") {
            let trimmed = ed.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        system_default.to_string()
    }
}

fn strip_toml_comment(val: &str) -> &str {
    let mut in_double = false;
    let mut in_single = false;
    let mut escaped = false;
    for (idx, ch) in val.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_double {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '#' && !in_double && !in_single {
            return &val[..idx];
        }
    }
    val
}

fn unquote_str(s: &str) -> &str {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn parse_toml_string_array(raw: &str) -> Option<Vec<String>> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let mut items = Vec::new();
    for part in inner.split(',') {
        let p = unquote_str(part).trim();
        if !p.is_empty() {
            items.push(p.to_string());
        }
    }
    Some(items)
}

fn format_toml_string_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }
    let quoted: Vec<String> = items.iter().map(|s| format!("\"{}\"", s)).collect();
    format!("[{}]", quoted.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip() {
        let cfg = AppConfig {
            editor: Some("code --wait".to_string()),
            lang: Some("en".to_string()),
            theme: Some("nord".to_string()),
            plugins: PluginsConfig::default(),
        };
        let text = cfg.to_config_string();
        let parsed = AppConfig::parse(&text);
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn test_plugins_config_defaults_and_custom_parsing() {
        // Empty config should deserialize with default [plugins] values
        let empty = AppConfig::parse("");
        assert_eq!(empty.plugins.dir, "~/.local/share/yarube/plugins");
        assert_eq!(empty.plugins.on_reflect, vec!["coach".to_string()]);
        assert!(empty.plugins.on_task_update.is_empty());
        assert!(empty.plugins.is_plugin_enabled("coach"));

        // Custom [plugins] and [plugins.coach] section
        let custom_toml = r#"
[general]
theme = "monokai"

[plugins]
dir = "/tmp/my-plugins"
on_reflect = ["coach", "logger"]
on_task_update = ["sync"]

[plugins.coach]
enabled = false
"#;
        let parsed = AppConfig::parse(custom_toml);
        assert_eq!(parsed.theme, Some("monokai".to_string()));
        assert_eq!(parsed.plugins.dir, "/tmp/my-plugins");
        assert_eq!(
            parsed.plugins.on_reflect,
            vec!["coach".to_string(), "logger".to_string()]
        );
        assert_eq!(parsed.plugins.on_task_update, vec!["sync".to_string()]);
        assert!(!parsed.plugins.is_plugin_enabled("coach"));
        assert!(parsed.plugins.is_plugin_enabled("logger"));

        // Tilde expansion test
        let expanded = expand_tilde("~/my_plugins");
        if let Ok(home) = env::var("HOME") {
            assert_eq!(expanded, PathBuf::from(home).join("my_plugins"));
        }

        // Partial [plugins] section without `on_reflect` or `dir` must retain defaults (`~/.local/share/yarube/plugins` and `["coach"]`)
        let partial_plugins_toml = r#"
[general]
theme = "nord"

[plugins]
on_task_update = ["sync"]
"#;
        let partial_parsed = AppConfig::parse(partial_plugins_toml);
        assert_eq!(partial_parsed.plugins.dir, "~/.local/share/yarube/plugins");
        assert_eq!(partial_parsed.plugins.on_reflect, vec!["coach".to_string()]);
    }

    #[test]
    fn test_editor_priority_resolution() {
        let cfg_with_ed = AppConfig {
            editor: Some("code --wait".to_string()),
            lang: None,
            theme: None,
            plugins: PluginsConfig::default(),
        };
        let resolved = cfg_with_ed.resolve_editor_with(
            |k| match k {
                "VISUAL" => Some("nvim".to_string()),
                "EDITOR" => Some("vim".to_string()),
                _ => None,
            },
            "nano",
        );
        assert_eq!(resolved, "code --wait");

        let empty_cfg = AppConfig::default();
        let resolved2 = empty_cfg.resolve_editor_with(
            |k| match k {
                "VISUAL" => Some("nvim".to_string()),
                "EDITOR" => Some("vim".to_string()),
                _ => None,
            },
            "nano",
        );
        assert_eq!(resolved2, "nvim");

        let resolved3 = empty_cfg.resolve_editor_with(
            |k| match k {
                "EDITOR" => Some("vim".to_string()),
                _ => None,
            },
            "nano",
        );
        assert_eq!(resolved3, "vim");

        let resolved4 = empty_cfg.resolve_editor_with(|_| None, "nano");
        assert_eq!(resolved4, "nano");
    }
}
