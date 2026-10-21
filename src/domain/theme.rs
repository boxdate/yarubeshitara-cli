#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    TokyoNight,
    Monokai,
    Nord,
    Sepia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeResolution {
    pub theme: ThemeName,
    pub unknown_requested: Option<String>,
}

pub struct ThemePreset {
    pub number: usize,
    pub name: ThemeName,
    pub id: &'static str,
    pub label_ja: &'static str,
    pub label_en: &'static str,
}

pub const THEME_PRESETS: &[ThemePreset] = &[
    ThemePreset {
        number: 1,
        name: ThemeName::TokyoNight,
        id: "tokyo-night",
        label_ja: "tokyo-night (デフォルト・落ち着いたダークブルー)",
        label_en: "tokyo-night (Default calm dark blue)",
    },
    ThemePreset {
        number: 2,
        name: ThemeName::Monokai,
        id: "monokai",
        label_ja: "monokai (鮮やかなコントラストの定番ダーク)",
        label_en: "monokai (Vibrant high-contrast classic)",
    },
    ThemePreset {
        number: 3,
        name: ThemeName::Nord,
        id: "nord",
        label_ja: "nord (北欧風のクールなアイスブルー)",
        label_en: "nord (Arctic cool pastel blue)",
    },
    ThemePreset {
        number: 4,
        name: ThemeName::Sepia,
        id: "sepia",
        label_ja: "sepia (紙の手帳・クラフト紙のような温かみ)",
        label_en: "sepia (Warm craft-paper notebook style)",
    },
];

impl ThemeName {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeName::TokyoNight => "tokyo-night",
            ThemeName::Monokai => "monokai",
            ThemeName::Nord => "nord",
            ThemeName::Sepia => "sepia",
        }
    }

    pub fn from_name(input: &str) -> Option<Self> {
        let normalized = input
            .trim()
            .to_lowercase()
            .replace(['_', ' '], "-");
        match normalized.as_str() {
            "tokyo-night" | "tokyonight" | "default" => Some(ThemeName::TokyoNight),
            "monokai" => Some(ThemeName::Monokai),
            "nord" => Some(ThemeName::Nord),
            "sepia" | "craft" | "sepia-craft" | "sepia/craft" => Some(ThemeName::Sepia),
            _ => None,
        }
    }

    /// Resolves a configured/requested theme string.
    /// If an unknown theme name is specified, returns `ThemeName::TokyoNight` along with `unknown_requested` for warning output (EARS-21).
    pub fn resolve(configured: Option<&str>) -> ThemeResolution {
        match configured {
            None => ThemeResolution {
                theme: ThemeName::TokyoNight,
                unknown_requested: None,
            },
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    ThemeResolution {
                        theme: ThemeName::TokyoNight,
                        unknown_requested: None,
                    }
                } else if let Some(valid) = Self::from_name(trimmed) {
                    ThemeResolution {
                        theme: valid,
                        unknown_requested: None,
                    }
                } else {
                    ThemeResolution {
                        theme: ThemeName::TokyoNight,
                        unknown_requested: Some(trimmed.to_string()),
                    }
                }
            }
        }
    }

    pub fn palette(&self) -> ThemePalette {
        match self {
            ThemeName::TokyoNight => ThemePalette {
                date_header: StyleSpec::bold(0x7a, 0xa2, 0xf7, "#7aa2f7"),
                section_heading: StyleSpec::bold(0xbb, 0x9a, 0xf7, "#bb9af7"),
                todo_task: StyleSpec::normal(0xc0, 0xca, 0xf5, "#c0caf5"),
                done_task: StyleSpec::dim_strike(0x56, 0x5f, 0x89, "#565f89"),
                migrated_task: StyleSpec::normal(0xe0, 0xaf, 0x68, "#e0af68"),
                cancelled_task: StyleSpec::dim_strike(0x56, 0x5f, 0x89, "#565f89"),
                subtask: StyleSpec::normal(0x9a, 0xa5, 0xce, "#9aa5ce"),
                reflection_body: StyleSpec::normal(0xc0, 0xca, 0xf5, "#c0caf5"),
                new_badge: StyleSpec::bold(0x9e, 0xce, 0x6a, "#9ece6a"),
                divider: StyleSpec::dim(0x3b, 0x42, 0x61, "#3b4261"),
            },
            ThemeName::Monokai => ThemePalette {
                date_header: StyleSpec::bold(0x66, 0xd9, 0xef, "#66d9ef"),
                section_heading: StyleSpec::bold(0xfd, 0x97, 0x1f, "#fd971f"),
                todo_task: StyleSpec::normal(0xf8, 0xf8, 0xf2, "#f8f8f2"),
                done_task: StyleSpec::dim_strike(0x75, 0x71, 0x5e, "#75715e"),
                migrated_task: StyleSpec::normal(0xe6, 0xdb, 0x74, "#e6db74"),
                cancelled_task: StyleSpec::dim_strike(0x75, 0x71, 0x5e, "#75715e"),
                subtask: StyleSpec::normal(0xa6, 0xe2, 0x2e, "#a6e22e"),
                reflection_body: StyleSpec::normal(0xf8, 0xf8, 0xf2, "#f8f8f2"),
                new_badge: StyleSpec::bold(0xa6, 0xe2, 0x2e, "#a6e22e"),
                divider: StyleSpec::dim(0x49, 0x48, 0x3e, "#49483e"),
            },
            ThemeName::Nord => ThemePalette {
                date_header: StyleSpec::bold(0x88, 0xc0, 0xd0, "#88c0d0"),
                section_heading: StyleSpec::bold(0x81, 0xa1, 0xc1, "#81a1c1"),
                todo_task: StyleSpec::normal(0xec, 0xef, 0xf4, "#eceff4"),
                done_task: StyleSpec::dim_strike(0x4c, 0x56, 0x6a, "#4c566a"),
                migrated_task: StyleSpec::normal(0xeb, 0xcb, 0x8b, "#ebcb8b"),
                cancelled_task: StyleSpec::dim_strike(0x4c, 0x56, 0x6a, "#4c566a"),
                subtask: StyleSpec::normal(0xd8, 0xde, 0xe9, "#d8dee9"),
                reflection_body: StyleSpec::normal(0xec, 0xef, 0xf4, "#eceff4"),
                new_badge: StyleSpec::bold(0xa3, 0xbe, 0x8c, "#a3be8c"),
                divider: StyleSpec::dim(0x3b, 0x42, 0x52, "#3b4252"),
            },
            ThemeName::Sepia => ThemePalette {
                date_header: StyleSpec::bold(0xd4, 0xa3, 0x73, "#d4a373"),
                section_heading: StyleSpec::bold(0xbc, 0x6c, 0x25, "#bc6c25"),
                todo_task: StyleSpec::normal(0xfa, 0xed, 0xcd, "#faedcd"),
                done_task: StyleSpec::dim_strike(0x8c, 0x82, 0x75, "#8c8275"),
                migrated_task: StyleSpec::normal(0xb0, 0x89, 0x68, "#b08968"),
                cancelled_task: StyleSpec::dim_strike(0x8c, 0x82, 0x75, "#8c8275"),
                subtask: StyleSpec::normal(0xdd, 0xb8, 0x92, "#ddb892"),
                reflection_body: StyleSpec::normal(0xfa, 0xed, 0xcd, "#faedcd"),
                new_badge: StyleSpec::bold(0x6b, 0x70, 0x5c, "#6b705c"),
                divider: StyleSpec::dim(0xd5, 0xbd, 0xaf, "#d5bdaf"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    Normal,
    Bold,
    Dim,
    DimStrikethrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleSpec {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: &'static str,
    pub style: TextStyle,
}

impl StyleSpec {
    pub const fn normal(r: u8, g: u8, b: u8, hex: &'static str) -> Self {
        Self {
            r,
            g,
            b,
            hex,
            style: TextStyle::Normal,
        }
    }

    pub const fn bold(r: u8, g: u8, b: u8, hex: &'static str) -> Self {
        Self {
            r,
            g,
            b,
            hex,
            style: TextStyle::Bold,
        }
    }

    pub const fn dim(r: u8, g: u8, b: u8, hex: &'static str) -> Self {
        Self {
            r,
            g,
            b,
            hex,
            style: TextStyle::Dim,
        }
    }

    pub const fn dim_strike(r: u8, g: u8, b: u8, hex: &'static str) -> Self {
        Self {
            r,
            g,
            b,
            hex,
            style: TextStyle::DimStrikethrough,
        }
    }

    /// Generates the 24-bit True Color ANSI escape prefix for this style specification.
    pub fn ansi_prefix(&self) -> String {
        match self.style {
            TextStyle::Normal => format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b),
            TextStyle::Bold => format!("\x1b[1;38;2;{};{};{}m", self.r, self.g, self.b),
            TextStyle::Dim => format!("\x1b[2;38;2;{};{};{}m", self.r, self.g, self.b),
            TextStyle::DimStrikethrough => {
                format!("\x1b[2;9;38;2;{};{};{}m", self.r, self.g, self.b)
            }
        }
    }

    /// Wraps `text` with this 24-bit True Color ANSI style and a trailing reset (`\x1b[0m`).
    pub fn paint(&self, text: &str) -> String {
        format!("{}{}\x1b[0m", self.ansi_prefix(), text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub date_header: StyleSpec,
    pub section_heading: StyleSpec,
    pub todo_task: StyleSpec,
    pub done_task: StyleSpec,
    pub migrated_task: StyleSpec,
    pub cancelled_task: StyleSpec,
    pub subtask: StyleSpec,
    pub reflection_body: StyleSpec,
    pub new_badge: StyleSpec,
    pub divider: StyleSpec,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_name_parsing_and_fallback() {
        assert_eq!(ThemeName::from_name("tokyo-night"), Some(ThemeName::TokyoNight));
        assert_eq!(ThemeName::from_name("Monokai"), Some(ThemeName::Monokai));
        assert_eq!(ThemeName::from_name("NORD"), Some(ThemeName::Nord));
        assert_eq!(ThemeName::from_name("sepia"), Some(ThemeName::Sepia));
        assert_eq!(ThemeName::from_name("unknown"), None);

        let res = ThemeName::resolve(Some("unknown"));
        assert_eq!(res.theme, ThemeName::TokyoNight);
        assert_eq!(res.unknown_requested, Some("unknown".to_string()));
    }

    #[test]
    fn test_monokai_palette_hex_and_ansi_codes() {
        let p = ThemeName::Monokai.palette();
        assert_eq!(p.date_header.hex, "#66d9ef");
        assert_eq!(p.date_header.ansi_prefix(), "\x1b[1;38;2;102;217;239m");

        assert_eq!(p.section_heading.hex, "#fd971f");
        assert_eq!(p.section_heading.ansi_prefix(), "\x1b[1;38;2;253;151;31m");

        assert_eq!(p.done_task.hex, "#75715e");
        assert_eq!(p.done_task.ansi_prefix(), "\x1b[2;9;38;2;117;113;94m");
    }
}
