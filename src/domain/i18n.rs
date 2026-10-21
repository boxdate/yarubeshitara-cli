use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Ja,
    En,
}

impl Language {
    pub fn from_code(s: &str) -> Option<Self> {
        let lower = s.trim().to_lowercase();
        if lower.starts_with("en") {
            Some(Language::En)
        } else if lower.starts_with("ja") || lower.starts_with("jp") {
            Some(Language::Ja)
        } else {
            None
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::Ja => "ja",
            Language::En => "en",
        }
    }

    /// Resolves language from optional config value and environment variables (`BUJO_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANG`).
    pub fn resolve(config_lang: Option<&str>) -> Self {
        Self::resolve_with(config_lang, |k| env::var(k).ok())
    }

    pub fn resolve_with<F>(config_lang: Option<&str>, get_env: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        if let Some(cfg) = config_lang {
            if let Some(lang) = Self::from_code(cfg) {
                return lang;
            }
        }
        for key in ["YARUBE_LANG", "BUJO_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Some(val) = get_env(key) {
                if let Some(lang) = Self::from_code(&val) {
                    return lang;
                }
            }
        }
        Language::Ja
    }

    pub fn todo_section_header(&self) -> &'static str {
        match self {
            Language::Ja => "#### △ やることリスト",
            Language::En => "#### △ Todo List",
        }
    }

    pub fn reflection_section_header(&self) -> &'static str {
        match self {
            Language::Ja => "#### △ ふりかえり",
            Language::En => "#### △ Reflection",
        }
    }

    /// Checks if a trimmed Markdown heading line represents the Todo section in either Japanese or English (EARS-17).
    pub fn is_todo_heading(trimmed_line: &str) -> bool {
        let Some(rest) = trimmed_line.strip_prefix("####") else {
            return false;
        };
        let title = rest.trim().trim_start_matches('△').trim();
        title == "やることリスト"
            || title.eq_ignore_ascii_case("todo list")
            || title.eq_ignore_ascii_case("todo")
    }

    /// Checks if a trimmed Markdown heading line represents the Reflection section in either Japanese or English (EARS-17).
    pub fn is_reflection_heading(trimmed_line: &str) -> bool {
        let Some(rest) = trimmed_line.strip_prefix("####") else {
            return false;
        };
        let title = rest.trim().trim_start_matches('△').trim();
        title == "ふりかえり"
            || title.eq_ignore_ascii_case("reflection")
            || title.eq_ignore_ascii_case("reflections")
    }

    pub fn review_no_pending_tasks(&self) -> &'static str {
        match self {
            Language::Ja => "* 未完了のタスクはありません。今日もお疲れさまでした！",
            Language::En => "* No pending tasks found! Great job today!",
        }
    }

    pub fn review_intro(&self, count: usize) -> String {
        match self {
            Language::Ja => format!("* 本日の未完了タスク（全{}件）を棚卸しします。", count),
            Language::En => format!("* Reviewing {} pending task(s) for today.", count),
        }
    }

    pub fn review_keys_guide(&self) -> &'static str {
        match self {
            Language::Ja => {
                "操作キー: [o] 完了(O)  [m/>] 翌日へ移行(>)  [c/~] 取消(~)  [s/Enter] スキップ(•)  [q/Ctrl+C] 中断"
            }
            Language::En => {
                "Keys: [o] Done(O)  [m/>] Migrate(>)  [c/~] Cancel(~)  [s/Enter] Skip(•)  [q/Ctrl+C] Quit"
            }
        }
    }

    pub fn review_prompt(&self) -> &'static str {
        match self {
            Language::Ja => "状態を選択 [o:完了(O) / m:移行(>) / c:取消(~) / s:スキップ]: ",
            Language::En => "Select status [o:Done(O) / m:Migrate(>) / c:Cancel(~) / s:Skip]: ",
        }
    }

    pub fn review_status_done(&self) -> &'static str {
        match self {
            Language::Ja => "O (完了)",
            Language::En => "O (Done)",
        }
    }

    pub fn review_status_migrate(&self) -> &'static str {
        match self {
            Language::Ja => "> (翌日へ移行)",
            Language::En => "> (Migrated to tomorrow)",
        }
    }

    pub fn review_status_cancel(&self) -> &'static str {
        match self {
            Language::Ja => "~ (取消)",
            Language::En => "~ (Cancelled)",
        }
    }

    pub fn review_status_skip(&self) -> &'static str {
        match self {
            Language::Ja => "• (スキップ)",
            Language::En => "• (Skipped)",
        }
    }

    pub fn review_interrupted(&self) -> &'static str {
        match self {
            Language::Ja => "\n[!] レビューを中断しました（直前までの更新は保存されています）。",
            Language::En => "\n[!] Review interrupted (all confirmed updates have been saved).",
        }
    }

    pub fn review_completed(&self) -> &'static str {
        match self {
            Language::Ja => "* タスクの棚卸しが完了しました！",
            Language::En => "* Task review completed!",
        }
    }

    pub fn review_ask_reflect(&self) -> &'static str {
        match self {
            Language::Ja => "\n続けて『△ ふりかえり』を記入しますか？ [y/N]: ",
            Language::En => "\nWould you like to write a reflection now? [y/N]: ",
        }
    }

    pub fn reflect_empty_skipped(&self) -> &'static str {
        match self {
            Language::Ja => "ふりかえりの入力が空のため、追記をスキップしました。",
            Language::En => "Reflection input was empty; skipped appending.",
        }
    }

    pub fn theme_unknown_warning(&self, unknown: &str) -> String {
        match self {
            Language::Ja => format!(
                "[!] 警告: 未定義のテーマ '{}' が指定されたため、デフォルトの 'tokyo-night' を適用します。",
                unknown
            ),
            Language::En => format!(
                "[!] Warning: Unknown theme '{}' specified. Falling back to default theme 'tokyo-night'.",
                unknown
            ),
        }
    }

    pub fn onboarding_lines(&self) -> [&'static str; 6] {
        match self {
            Language::Ja => [
                "* 今日の手帳を用意しました！以下のコマンドで記録を始められます：",
                "  yarube タスク内容          : 『△ やることリスト』にタスク(•)を追記",
                "  yarube sub 補足内容        : 直前のタスクにサブ項目(  - )を追記",
                "  yarube review              : 未完了タスクを1問1答で棚卸し(O / > / ~)",
                "  yarube reflect             : エディタを開いて『△ ふりかえり』を記述",
                "  yarube theme <name>        : カラーテーマ切替 (tokyo-night / monokai / nord / sepia)",
            ],
            Language::En => [
                "* Today's journal is ready! Get started with these commands:",
                "  yarube <task text>         : Append a task (•) to '△ Todo List'",
                "  yarube sub <note>          : Append an indented subtask (  - ) under the last task",
                "  yarube review              : Interactively review pending tasks (O / > / ~)",
                "  yarube reflect             : Open your editor to write in '△ Reflection'",
                "  yarube theme <name>        : Switch color theme (tokyo-night / monokai / nord / sepia)",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_resolution_from_env_and_config() {
        let lang = Language::resolve_with(None, |k| match k {
            "LANG" => Some("en_US.UTF-8".to_string()),
            _ => None,
        });
        assert_eq!(lang, Language::En);

        let lang_ja = Language::resolve_with(None, |k| match k {
            "LANG" => Some("ja_JP.UTF-8".to_string()),
            _ => None,
        });
        assert_eq!(lang_ja, Language::Ja);

        let lang_cfg = Language::resolve_with(Some("en"), |k| match k {
            "LANG" => Some("ja_JP.UTF-8".to_string()),
            _ => None,
        });
        assert_eq!(lang_cfg, Language::En);
    }

    #[test]
    fn test_bilingual_heading_detection() {
        assert!(Language::is_todo_heading("#### △ やることリスト"));
        assert!(Language::is_todo_heading("#### △ Todo List"));
        assert!(Language::is_todo_heading("#### Todo List"));
        assert!(!Language::is_todo_heading("#### △ ふりかえり"));

        assert!(Language::is_reflection_heading("#### △ ふりかえり"));
        assert!(Language::is_reflection_heading("#### △ Reflection"));
        assert!(Language::is_reflection_heading("#### Reflection"));
        assert!(!Language::is_reflection_heading("#### △ Todo List"));
    }
}
