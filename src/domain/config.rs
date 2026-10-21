use crate::domain::i18n::Language;
use crate::domain::theme::{ThemeName, ThemeResolution};
use std::env;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub editor: Option<String>,
    pub lang: Option<String>,
    pub theme: Option<String>,
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
        let mut current_section = "";

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line.trim_matches(&['[', ']'][..]).trim();
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let k = key.trim();
                let v = val
                    .split('#')
                    .next()
                    .unwrap_or(val)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim();
                if v.is_empty() {
                    continue;
                }
                match (current_section, k) {
                    (_, "editor") => editor = Some(v.to_string()),
                    (_, "lang" | "language") => lang = Some(v.to_string()),
                    (_, "theme") | ("theme", "name") => theme = Some(v.to_string()),
                    _ => {}
                }
            }
        }
        Self {
            editor,
            lang,
            theme,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip() {
        let cfg = AppConfig {
            editor: Some("code --wait".to_string()),
            lang: Some("en".to_string()),
            theme: Some("nord".to_string()),
        };
        let text = cfg.to_config_string();
        let parsed = AppConfig::parse(&text);
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn test_editor_priority_resolution() {
        let cfg_with_ed = AppConfig {
            editor: Some("code --wait".to_string()),
            lang: None,
            theme: None,
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
