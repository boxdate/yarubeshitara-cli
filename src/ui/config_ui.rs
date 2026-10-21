use crate::domain::config::EDITOR_PRESETS;
use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::domain::journal::HighlightTarget;
use crate::domain::theme::{ThemeName, THEME_PRESETS};
use crate::ports::{ConfigRepository, JournalRepository};
use crate::ui::renderer::render_daily_log_with_options;
use std::io::{self, BufRead, Write};

/// Interactive or direct editor configuration (`bujo config editor [cmd]`) (TP-13 / EARS-13).
pub fn run_config_editor<C: ConfigRepository>(
    config_repo: &C,
    direct_editor: Option<String>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();
    run_config_editor_with_io(config_repo, direct_editor, &mut reader, &mut stdout)
}

pub fn run_config_editor_with_io<C, R, W>(
    config_repo: &C,
    direct_editor: Option<String>,
    reader: &mut R,
    out: &mut W,
) -> io::Result<()>
where
    C: ConfigRepository,
    R: BufRead,
    W: Write,
{
    let mut cfg = config_repo.load_config()?;
    let lang = cfg.resolve_language();

    if let Some(cmd) = direct_editor {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                match lang {
                    Language::Ja => "エディタコマンドが空です。",
                    Language::En => "Editor command cannot be empty.",
                },
            ));
        }
        cfg.editor = Some(trimmed.to_string());
        config_repo.save_config(&cfg)?;
        match lang {
            Language::Ja => writeln!(out, "* エディタを `{}` に設定しました。", trimmed)?,
            Language::En => writeln!(out, "* Editor set to `{}`.", trimmed)?,
        }
        return Ok(());
    }

    let current_effective = cfg.resolve_editor();
    match lang {
        Language::Ja => {
            writeln!(out, "* ふりかえり（yarube reflect）で使用するエディタを設定します。")?;
            match &cfg.editor {
                Some(ed) => writeln!(out, "現在の設定: `{}`", ed)?,
                None => writeln!(out, "現在の設定: 未設定（有効なエディタ: `{}`）", current_effective)?,
            }
        }
        Language::En => {
            writeln!(out, "* Configure editor for reflections (yarube reflect).")?;
            match &cfg.editor {
                Some(ed) => writeln!(out, "Current setting: `{}`", ed)?,
                None => writeln!(out, "Current setting: Not set (Effective editor: `{}`)", current_effective)?,
            }
        }
    }
    writeln!(out)?;
    for preset in EDITOR_PRESETS {
        writeln!(out, "  {}) {}", preset.number, preset.label(lang))?;
    }
    let custom_num = EDITOR_PRESETS.len() + 1;
    match lang {
        Language::Ja => {
            writeln!(out, "  {}) カスタムコマンドを直接入力", custom_num)?;
            writeln!(out)?;
            write!(
                out,
                "番号（1-{}）またはエディタコマンドを入力してください: ",
                custom_num
            )?;
        }
        Language::En => {
            writeln!(out, "  {}) Enter custom editor command", custom_num)?;
            writeln!(out)?;
            write!(
                out,
                "Enter number (1-{}) or editor command: ",
                custom_num
            )?;
        }
    }
    out.flush()?;

    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        writeln!(out)?;
        return Ok(());
    }
    let choice = line.trim();
    if choice.is_empty() {
        return Ok(());
    }

    let selected_editor = if let Ok(num) = choice.parse::<usize>() {
        if let Some(preset) = EDITOR_PRESETS.iter().find(|p| p.number == num) {
            preset.command.to_string()
        } else if num == custom_num {
            match lang {
                Language::Ja => write!(
                    out,
                    "使用するエディタコマンドを入力してください (例: code --wait): "
                )?,
                Language::En => write!(
                    out,
                    "Enter custom editor command (e.g. code --wait): "
                )?,
            }
            out.flush()?;
            let mut custom_line = String::new();
            reader.read_line(&mut custom_line)?;
            let custom = custom_line.trim();
            if custom.is_empty() {
                return Ok(());
            }
            custom.to_string()
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid selection: {}", num),
            ));
        }
    } else {
        choice.to_string()
    };

    cfg.editor = Some(selected_editor.clone());
    config_repo.save_config(&cfg)?;
    match lang {
        Language::Ja => writeln!(out, "* エディタを `{}` に設定しました。", selected_editor)?,
        Language::En => writeln!(out, "* Editor set to `{}`.", selected_editor)?,
    }
    Ok(())
}

/// Interactive or direct display language configuration (`bujo config lang [ja|en]`) (AC-10).
pub fn run_config_lang<C: ConfigRepository>(
    config_repo: &C,
    direct_lang: Option<String>,
) -> io::Result<()> {
    let mut cfg = config_repo.load_config()?;

    let target_lang = if let Some(val) = direct_lang {
        Language::from_code(&val).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unsupported language `{}`. Use `ja` or `en`.", val),
            )
        })?
    } else {
        println!("* Select display language / 表示言語を選択してください:");
        println!("  1) ja - 日本語 (Japanese)");
        println!("  2) en - English");
        print!("Select [1/2 or ja/en]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        match line.trim() {
            "1" => Language::Ja,
            "2" => Language::En,
            other => Language::from_code(other).unwrap_or_else(|| cfg.resolve_language()),
        }
    };

    cfg.lang = Some(target_lang.code().to_string());
    config_repo.save_config(&cfg)?;
    match target_lang {
        Language::Ja => println!("* 表示言語を日本語 (ja) に設定しました。"),
        Language::En => println!("* Display language set to English (en)."),
    }
    Ok(())
}

/// Switches color theme (`bujo theme [name]`), persists to config file, and immediately previews today's journal (AC-12 / TP-20, TP-21).
pub fn run_config_theme<R, C>(
    journal_repo: &R,
    config_repo: &C,
    today: LocalDate,
    direct_theme: Option<String>,
) -> io::Result<()>
where
    R: JournalRepository,
    C: ConfigRepository,
{
    let mut cfg = config_repo.load_config()?;
    let lang = cfg.resolve_language();

    let selected_input = if let Some(val) = direct_theme {
        val
    } else {
        let current = cfg.resolve_theme().theme;
        match lang {
            Language::Ja => {
                println!("* カラーテーマを選択してください（現在: `{}`）:", current.as_str());
                for p in THEME_PRESETS {
                    println!("  {}) {}", p.number, p.label_ja);
                }
                print!("番号（1-4）またはテーマ名を入力してください: ");
            }
            Language::En => {
                println!(
                    "* Select a color theme (Current: `{}`):",
                    current.as_str()
                );
                for p in THEME_PRESETS {
                    println!("  {}) {}", p.number, p.label_en);
                }
                print!("Enter number (1-4) or theme name: ");
            }
        }
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if let Ok(num) = trimmed.parse::<usize>() {
            if let Some(p) = THEME_PRESETS.iter().find(|p| p.number == num) {
                p.id.to_string()
            } else {
                trimmed.to_string()
            }
        } else {
            trimmed.to_string()
        }
    };

    let resolution = ThemeName::resolve(Some(&selected_input));
    if let Some(ref unknown) = resolution.unknown_requested {
        println!("{}", lang.theme_unknown_warning(unknown));
    }

    let applied_theme = resolution.theme;
    cfg.theme = Some(applied_theme.as_str().to_string());
    config_repo.save_config(&cfg)?;

    match lang {
        Language::Ja => println!("* テーマを `{}` に設定しました。", applied_theme.as_str()),
        Language::En => println!("* Theme set to `{}`.", applied_theme.as_str()),
    }
    println!();

    // Immediately redraw today's journal with the newly applied theme palette (EARS-20)
    let res = journal_repo.load_or_init(today)?;
    render_daily_log_with_options(
        &res.log,
        HighlightTarget::None,
        res.newly_created,
        lang,
        applied_theme,
    );
    Ok(())
}
