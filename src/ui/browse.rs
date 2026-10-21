use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::domain::journal::HighlightTarget;
use crate::domain::navigation::resolve_relative_recorded_date;
use crate::domain::theme::ThemeName;
use crate::ports::JournalRepository;
use crate::ui::renderer::render_daily_log_with_options;
use crate::ui::term::{RawTermGuard, INTERRUPTED};
use std::io::{self, Read, Write};
use std::sync::atomic::Ordering;

/// Displays the recorded daily log at `offset` relative to `base_date` (AC-08 / TP-14 / EARS-14).
pub fn run_navigate_relative<R: JournalRepository>(
    journal_repo: &R,
    base_date: LocalDate,
    offset: isize,
    lang: Language,
    theme: ThemeName,
) -> io::Result<()> {
    let recorded = journal_repo.list_recorded_dates()?;
    match resolve_relative_recorded_date(&recorded, base_date, offset) {
        Some(target_date) => {
            if let Some(log) = journal_repo.load_existing(target_date)? {
                render_daily_log_with_options(&log, HighlightTarget::None, false, lang, theme);
            } else {
                match lang {
                    Language::Ja => println!(
                        "* {} の記録が見つかりませんでした。",
                        target_date.format_ymd()
                    ),
                    Language::En => println!("* No log found for {}.", target_date.format_ymd()),
                }
            }
        }
        None => match (lang, offset < 0) {
            (Language::Ja, true) => println!(
                "* {} より前の記録日（{}ページ前）は見つかりませんでした。",
                base_date.format_ymd(),
                -offset
            ),
            (Language::Ja, false) => println!(
                "* {} より後の記録日（+{}ページ先）は見つかりませんでした。",
                base_date.format_ymd(),
                offset
            ),
            (Language::En, true) => println!(
                "* No recorded log found {} page(s) before {}.",
                -offset,
                base_date.format_ymd()
            ),
            (Language::En, false) => println!(
                "* No recorded log found +{} page(s) after {}.",
                offset,
                base_date.format_ymd()
            ),
        },
    }
    Ok(())
}

/// Interactive page-turning mode across recorded daily logs (`bujo browse`).
pub fn run_browse<R: JournalRepository>(
    journal_repo: &R,
    today: LocalDate,
    lang: Language,
    theme: ThemeName,
) -> io::Result<()> {
    let mut dates = journal_repo.list_recorded_dates()?;
    if dates.is_empty() {
        let res = journal_repo.load_or_init(today)?;
        render_daily_log_with_options(&res.log, HighlightTarget::None, res.newly_created, lang, theme);
        return Ok(());
    }
    dates.sort();

    let mut idx = dates
        .iter()
        .position(|&d| d == today)
        .unwrap_or_else(|| dates.len() - 1);

    let guard = RawTermGuard::new();
    let is_tty = guard.is_tty();
    let mut stdin = io::stdin();
    let divider = theme
        .palette()
        .divider
        .paint("────────────────────────────────────────");

    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            break;
        }
        let current_date = dates[idx];
        match lang {
            Language::Ja => println!(
                "\n* ページ [{}/{}] ({})",
                idx + 1,
                dates.len(),
                current_date.format_ymd()
            ),
            Language::En => println!(
                "\n* Page [{}/{}] ({})",
                idx + 1,
                dates.len(),
                current_date.format_ymd()
            ),
        }
        println!("{}", divider);
        if let Some(log) = journal_repo.load_existing(current_date)? {
            render_daily_log_with_options(&log, HighlightTarget::None, false, lang, theme);
        }
        println!("{}", divider);
        match lang {
            Language::Ja => {
                print!("操作: [p/h/←] 前の記録日  [n/l/→] 次の記録日  [t] 最新へ  [q] 終了: ")
            }
            Language::En => {
                print!("Keys: [p/h/←] Prev day  [n/l/→] Next day  [t] Latest  [q] Quit: ")
            }
        }
        io::stdout().flush()?;

        match read_browse_action(&mut stdin, is_tty)? {
            BrowseAction::Prev => {
                if idx > 0 {
                    idx -= 1;
                } else {
                    match lang {
                        Language::Ja => println!(" (これより前の記録はありません)"),
                        Language::En => println!(" (No earlier records)"),
                    }
                }
            }
            BrowseAction::Next => {
                if idx + 1 < dates.len() {
                    idx += 1;
                } else {
                    match lang {
                        Language::Ja => println!(" (これより後の記録はありません)"),
                        Language::En => println!(" (No later records)"),
                    }
                }
            }
            BrowseAction::Latest => {
                idx = dates.len() - 1;
            }
            BrowseAction::Quit => {
                println!();
                break;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowseAction {
    Prev,
    Next,
    Latest,
    Quit,
}

fn read_browse_action(stdin: &mut io::Stdin, _is_tty: bool) -> io::Result<BrowseAction> {
    let mut buf = [0u8; 1];
    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            return Ok(BrowseAction::Quit);
        }
        let n = match stdin.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => return Ok(BrowseAction::Quit),
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(BrowseAction::Quit);
        }
        match buf[0] {
            0x03 | b'q' | b'Q' => return Ok(BrowseAction::Quit),
            b'p' | b'P' | b'h' | b'H' => return Ok(BrowseAction::Prev),
            b'n' | b'N' | b'l' | b'L' => return Ok(BrowseAction::Next),
            b't' | b'T' => return Ok(BrowseAction::Latest),
            0x1b => {
                let mut seq = [0u8; 2];
                if stdin.read(&mut seq).unwrap_or(0) == 2 && seq[0] == b'[' {
                    match seq[1] {
                        b'D' => return Ok(BrowseAction::Prev),
                        b'C' => return Ok(BrowseAction::Next),
                        _ => {}
                    }
                } else {
                    return Ok(BrowseAction::Quit);
                }
            }
            _ => {}
        }
    }
}
