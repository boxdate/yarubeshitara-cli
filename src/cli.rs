#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    ShowToday,
    AddTask(String),
    AddSubtask(String),
    Review,
    Reflect(Option<String>),
    ConfigEditor(Option<String>),
    ConfigLang(Option<String>),
    Theme(Option<String>),
    NavigateRelative(isize),
    Browse,
    Search(String),
    Help,
}

/// Pure function that parses CLI arguments (excluding binary name) into a `CliCommand`.
pub fn parse_args(args: &[String]) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Ok(CliCommand::ShowToday);
    }

    let first = args[0].as_str();

    match first {
        "-h" | "--help" | "help" => return Ok(CliCommand::Help),
        "review" if args.len() == 1 => return Ok(CliCommand::Review),
        "browse" if args.len() == 1 => return Ok(CliCommand::Browse),
        "reflect" => {
            if args.len() == 1 {
                return Ok(CliCommand::Reflect(None));
            }
            let text = args[1..].join(" ");
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(CliCommand::Reflect(None));
            }
            return Ok(CliCommand::Reflect(Some(trimmed.to_string())));
        }
        "theme" => {
            if args.len() == 1 {
                return Ok(CliCommand::Theme(None));
            }
            let theme_val = args[1..].join(" ").trim().to_string();
            if theme_val.is_empty() {
                return Ok(CliCommand::Theme(None));
            }
            return Ok(CliCommand::Theme(Some(theme_val)));
        }
        "config" => {
            if args.len() == 1 {
                return Ok(CliCommand::ConfigEditor(None));
            }
            match args[1].as_str() {
                "editor" => {
                    if args.len() == 2 {
                        return Ok(CliCommand::ConfigEditor(None));
                    }
                    let editor_cmd = args[2..].join(" ").trim().to_string();
                    if editor_cmd.is_empty() {
                        return Ok(CliCommand::ConfigEditor(None));
                    }
                    return Ok(CliCommand::ConfigEditor(Some(editor_cmd)));
                }
                "lang" | "language" => {
                    if args.len() == 2 {
                        return Ok(CliCommand::ConfigLang(None));
                    }
                    let lang_val = args[2..].join(" ").trim().to_string();
                    if lang_val.is_empty() {
                        return Ok(CliCommand::ConfigLang(None));
                    }
                    return Ok(CliCommand::ConfigLang(Some(lang_val)));
                }
                "theme" => {
                    if args.len() == 2 {
                        return Ok(CliCommand::Theme(None));
                    }
                    let theme_val = args[2..].join(" ").trim().to_string();
                    if theme_val.is_empty() {
                        return Ok(CliCommand::Theme(None));
                    }
                    return Ok(CliCommand::Theme(Some(theme_val)));
                }
                other => {
                    return Err(format!(
                        "不明な config サブコマンドです: `{}`（使い方: `yarube config editor` / `yarube config lang` / `yarube theme <name>`）",
                        other
                    ));
                }
            }
        }
        "prev" => {
            if args.len() == 1 {
                return Ok(CliCommand::NavigateRelative(-1));
            }
            if args.len() == 2 {
                if let Ok(n) = args[1].parse::<isize>() {
                    if n > 0 {
                        return Ok(CliCommand::NavigateRelative(-n));
                    }
                }
            }
        }
        "next" => {
            if args.len() == 1 {
                return Ok(CliCommand::NavigateRelative(1));
            }
            if args.len() == 2 {
                if let Ok(n) = args[1].parse::<isize>() {
                    if n > 0 {
                        return Ok(CliCommand::NavigateRelative(n));
                    }
                }
            }
        }
        "search" => {
            let query = args[1..].join(" ");
            let trimmed = query.trim();
            if trimmed.is_empty() {
                return Err("検索キーワードを指定してください（例: yarube search EndeavourOS）".to_string());
            }
            return Ok(CliCommand::Search(trimmed.to_string()));
        }
        "sub" => {
            let text = args[1..].join(" ");
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err("サブ項目の内容を入力してください（例: yarube sub 補足内容）".to_string());
            }
            return Ok(CliCommand::AddSubtask(trimmed.to_string()));
        }
        "t" | "•" => {
            let text = args[1..].join(" ");
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err("タスクの内容を入力してください（例: yarube t タスク内容）".to_string());
            }
            return Ok(CliCommand::AddTask(trimmed.to_string()));
        }
        _ => {}
    }

    if args.len() == 1 && (first.starts_with('-') || first.starts_with('+')) && first.len() > 1 {
        if let Ok(offset) = first.parse::<isize>() {
            if offset != 0 {
                return Ok(CliCommand::NavigateRelative(offset));
            }
        }
    }

    let text = args.join(" ");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(CliCommand::ShowToday)
    } else {
        Ok(CliCommand::AddTask(trimmed.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn test_parse_basic_commands() {
        assert_eq!(parse_args(&s(&[])).unwrap(), CliCommand::ShowToday);
        assert_eq!(
            parse_args(&s(&["t", "タスク1"])).unwrap(),
            CliCommand::AddTask("タスク1".to_string())
        );
        assert_eq!(
            parse_args(&s(&["明日の", "会議", "資料作成"])).unwrap(),
            CliCommand::AddTask("明日の 会議 資料作成".to_string())
        );
        assert_eq!(
            parse_args(&s(&["sub", "Lubuntu -> Xubuntu"])).unwrap(),
            CliCommand::AddSubtask("Lubuntu -> Xubuntu".to_string())
        );
        assert_eq!(parse_args(&s(&["review"])).unwrap(), CliCommand::Review);
        assert_eq!(
            parse_args(&s(&["reflect"])).unwrap(),
            CliCommand::Reflect(None)
        );
    }

    #[test]
    fn test_parse_us03_us04_us05_us06_commands() {
        assert_eq!(
            parse_args(&s(&["config", "editor"])).unwrap(),
            CliCommand::ConfigEditor(None)
        );
        assert_eq!(
            parse_args(&s(&["config", "editor", "code", "--wait"])).unwrap(),
            CliCommand::ConfigEditor(Some("code --wait".to_string()))
        );
        assert_eq!(
            parse_args(&s(&["config", "lang", "en"])).unwrap(),
            CliCommand::ConfigLang(Some("en".to_string()))
        );
        assert_eq!(
            parse_args(&s(&["theme", "nord"])).unwrap(),
            CliCommand::Theme(Some("nord".to_string()))
        );
        assert_eq!(
            parse_args(&s(&["config", "theme", "monokai"])).unwrap(),
            CliCommand::Theme(Some("monokai".to_string()))
        );
        assert_eq!(
            parse_args(&s(&["prev"])).unwrap(),
            CliCommand::NavigateRelative(-1)
        );
        assert_eq!(
            parse_args(&s(&["-1"])).unwrap(),
            CliCommand::NavigateRelative(-1)
        );
        assert_eq!(
            parse_args(&s(&["-2"])).unwrap(),
            CliCommand::NavigateRelative(-2)
        );
        assert_eq!(
            parse_args(&s(&["next"])).unwrap(),
            CliCommand::NavigateRelative(1)
        );
        assert_eq!(
            parse_args(&s(&["+1"])).unwrap(),
            CliCommand::NavigateRelative(1)
        );
        assert_eq!(parse_args(&s(&["browse"])).unwrap(), CliCommand::Browse);
        assert_eq!(
            parse_args(&s(&["search", "EndeavourOS"])).unwrap(),
            CliCommand::Search("EndeavourOS".to_string())
        );
    }
}
