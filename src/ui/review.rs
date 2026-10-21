use crate::domain::journal::{DailyLog, HighlightTarget, TaskStatus};
use crate::ports::{ConfigRepository, EditorLauncher, JournalRepository};
use crate::ui::reflect::run_reflect;
use crate::ui::renderer::render_daily_log_with_options;
use crate::ui::term::{RawTermGuard, INTERRUPTED};
use std::io::{self, Read, Write};
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewChoice {
    Done,
    Migrate,
    Cancel,
    Skip,
    Interrupt,
}

/// Interactive 1-by-1 review of incomplete (`•`) tasks (AC-04, AC-05, AC-10).
/// Immediately saves after each confirmed task so Ctrl+C interruption retains all progress (EARS-08, EARS-09, EARS-10, EARS-16).
pub fn run_review<R, C, E>(
    journal_repo: &R,
    config_repo: &C,
    editor_launcher: &E,
    mut log: DailyLog,
) -> io::Result<()>
where
    R: JournalRepository,
    C: ConfigRepository,
    E: EditorLauncher,
{
    let cfg = config_repo.load_config().unwrap_or_default();
    let lang = cfg.resolve_language();
    let theme = cfg.resolve_theme().theme;
    let divider = theme
        .palette()
        .divider
        .paint("────────────────────────────────────────");

    let todo_indices: Vec<usize> = log
        .tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.status == TaskStatus::Todo)
        .map(|(i, _)| i)
        .collect();

    if todo_indices.is_empty() {
        println!("{}", lang.review_no_pending_tasks());
        return Ok(());
    }

    println!("{}", lang.review_intro(todo_indices.len()));
    println!("{}", lang.review_keys_guide());
    println!();

    let total = todo_indices.len();
    let is_tty;

    {
        let guard = RawTermGuard::new();
        is_tty = guard.is_tty();
        let mut stdin = io::stdin();
        let mut skip_trailing_newline = false;

        for (step, &task_idx) in todo_indices.iter().enumerate() {
            if INTERRUPTED.load(Ordering::SeqCst) {
                println!("{}", lang.review_interrupted());
                return Ok(());
            }

            let task = &log.tasks[task_idx];
            println!("{}", divider);
            println!("[{}/{}] • {}", step + 1, total, task.text);
            for sub in &task.subtasks {
                println!("      - {}", sub);
            }
            print!("{}", lang.review_prompt());
            io::stdout().flush()?;

            let choice = read_review_choice(&mut stdin, is_tty, &mut skip_trailing_newline)?;
            match choice {
                ReviewChoice::Done => {
                    println!("{}", lang.review_status_done());
                    log.tasks[task_idx].status = TaskStatus::Done;
                    journal_repo.save(&log)?;
                }
                ReviewChoice::Migrate => {
                    println!("{}", lang.review_status_migrate());
                    log.tasks[task_idx].status = TaskStatus::Migrated;
                    journal_repo.save(&log)?;
                    let migrated_task = log.tasks[task_idx].clone();
                    journal_repo.migrate_task_to_next_day(log.date, &migrated_task)?;
                }
                ReviewChoice::Cancel => {
                    println!("{}", lang.review_status_cancel());
                    log.tasks[task_idx].status = TaskStatus::Cancelled;
                    journal_repo.save(&log)?;
                }
                ReviewChoice::Skip => {
                    println!("{}", lang.review_status_skip());
                }
                ReviewChoice::Interrupt => {
                    println!("{}", lang.review_interrupted());
                    return Ok(());
                }
            }
        }
    }

    println!("{}", divider);
    println!("{}", lang.review_completed());
    println!();
    render_daily_log_with_options(&log, HighlightTarget::None, false, lang, theme);

    if is_tty && !INTERRUPTED.load(Ordering::SeqCst) {
        print!("{}", lang.review_ask_reflect());
        io::stdout().flush()?;
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_ok() {
            if matches!(answer.trim(), "y" | "Y" | "yes" | "YES") {
                run_reflect(journal_repo, config_repo, editor_launcher, log, None)?;
            }
        }
    }

    Ok(())
}

fn read_review_choice(
    stdin: &mut io::Stdin,
    is_tty: bool,
    skip_trailing_newline: &mut bool,
) -> io::Result<ReviewChoice> {
    let mut buf = [0u8; 1];
    loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            return Ok(ReviewChoice::Interrupt);
        }
        let n = match stdin.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                return Ok(ReviewChoice::Interrupt);
            }
            Err(e) => return Err(e),
        };
        if n == 0 {
            return Ok(ReviewChoice::Interrupt);
        }
        let ch = buf[0];

        if !is_tty && *skip_trailing_newline {
            if ch == b'\r' {
                continue;
            }
            *skip_trailing_newline = false;
            if ch == b'\n' {
                continue;
            }
        }

        match ch {
            0x03 | b'q' | b'Q' => return Ok(ReviewChoice::Interrupt),
            b'o' | b'O' | b'x' | b'X' => {
                *skip_trailing_newline = !is_tty;
                return Ok(ReviewChoice::Done);
            }
            b'm' | b'M' | b'>' => {
                *skip_trailing_newline = !is_tty;
                return Ok(ReviewChoice::Migrate);
            }
            b'c' | b'C' | b'~' | b'-' => {
                *skip_trailing_newline = !is_tty;
                return Ok(ReviewChoice::Cancel);
            }
            b's' | b'S' | b' ' => {
                *skip_trailing_newline = !is_tty;
                return Ok(ReviewChoice::Skip);
            }
            b'\r' => {
                *skip_trailing_newline = !is_tty;
                return Ok(ReviewChoice::Skip);
            }
            b'\n' => {
                *skip_trailing_newline = false;
                return Ok(ReviewChoice::Skip);
            }
            _ => {}
        }
    }
}
