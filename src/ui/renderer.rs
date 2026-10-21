use crate::domain::i18n::Language;
use crate::domain::journal::{DailyLog, HighlightTarget, TaskStatus};
use crate::domain::search::DaySearchResult;
use crate::domain::theme::ThemeName;
use std::io::{self, Write};

const ANSI_RESET: &str = "\x1b[0m";

/// Renders the entire `DailyLog` to stdout using the resolved environment/default language and default theme.
pub fn render_daily_log(log: &DailyLog, highlight: HighlightTarget, newly_created: bool) {
    render_daily_log_with_options(
        log,
        highlight,
        newly_created,
        Language::resolve(None),
        ThemeName::TokyoNight,
    );
}

pub fn render_daily_log_with_lang(
    log: &DailyLog,
    highlight: HighlightTarget,
    newly_created: bool,
    lang: Language,
) {
    render_daily_log_with_options(log, highlight, newly_created, lang, ThemeName::TokyoNight);
}

pub fn render_daily_log_with_options(
    log: &DailyLog,
    highlight: HighlightTarget,
    newly_created: bool,
    lang: Language,
    theme: ThemeName,
) {
    let mut out = io::stdout();
    let _ = render_daily_log_to_with_options(&mut out, log, highlight, newly_created, lang, theme);
}

/// Renders the entire `DailyLog` to an arbitrary `io::Write` sink (testable without stdout capture).
pub fn render_daily_log_to<W: Write + ?Sized>(
    out: &mut W,
    log: &DailyLog,
    highlight: HighlightTarget,
    newly_created: bool,
) -> io::Result<()> {
    render_daily_log_to_with_options(
        out,
        log,
        highlight,
        newly_created,
        Language::resolve(None),
        ThemeName::TokyoNight,
    )
}

pub fn render_daily_log_to_with_lang<W: Write + ?Sized>(
    out: &mut W,
    log: &DailyLog,
    highlight: HighlightTarget,
    newly_created: bool,
    lang: Language,
) -> io::Result<()> {
    render_daily_log_to_with_options(
        out,
        log,
        highlight,
        newly_created,
        lang,
        ThemeName::TokyoNight,
    )
}

pub fn render_daily_log_to_with_options<W: Write + ?Sized>(
    out: &mut W,
    log: &DailyLog,
    highlight: HighlightTarget,
    newly_created: bool,
    lang: Language,
    theme: ThemeName,
) -> io::Result<()> {
    let palette = theme.palette();

    writeln!(
        out,
        "{}",
        palette.date_header.paint(&log.date.format_header_for(lang))
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "{}",
        palette.section_heading.paint(lang.todo_section_header())
    )?;

    for (t_idx, task) in log.tasks.iter().enumerate() {
        let is_highlighted = highlight == HighlightTarget::Task(t_idx);
        let line = format!("{} {}", task.status.symbol(), task.text);

        if is_highlighted {
            writeln!(
                out,
                "{}",
                palette.new_badge.paint(&format!("{}  [NEW]", line))
            )?;
        } else {
            let styled = match task.status {
                TaskStatus::Todo => palette.todo_task.paint(&line),
                TaskStatus::Done => palette.done_task.paint(&line),
                TaskStatus::Migrated => palette.migrated_task.paint(&line),
                TaskStatus::Cancelled => palette.cancelled_task.paint(&line),
            };
            writeln!(out, "{}", styled)?;
        }

        for (s_idx, sub) in task.subtasks.iter().enumerate() {
            let sub_highlighted = highlight
                == HighlightTarget::Subtask {
                    task_index: t_idx,
                    sub_index: s_idx,
                };
            let sub_line = format!("  - {}", sub);
            if sub_highlighted {
                writeln!(
                    out,
                    "{}",
                    palette.new_badge.paint(&format!("{}  [NEW]", sub_line))
                )?;
            } else if task.status == TaskStatus::Cancelled || task.status == TaskStatus::Done {
                writeln!(out, "{}", palette.done_task.paint(&sub_line))?;
            } else {
                writeln!(out, "{}", palette.subtask.paint(&sub_line))?;
            }
        }
    }

    writeln!(out)?;
    writeln!(
        out,
        "{}",
        palette
            .section_heading
            .paint(lang.reflection_section_header())
    )?;
    for (idx, line) in log.reflection.iter().enumerate() {
        let is_last = idx + 1 == log.reflection.len();
        if highlight == HighlightTarget::Reflection && is_last {
            writeln!(
                out,
                "{}",
                palette.new_badge.paint(&format!("{}  [NEW]", line))
            )?;
        } else {
            writeln!(out, "{}", palette.reflection_body.paint(line))?;
        }
    }

    if newly_created || log.is_empty() {
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            palette
                .divider
                .paint("────────────────────────────────────────")
        )?;
        for guide_line in lang.onboarding_lines() {
            writeln!(out, "{}", palette.divider.paint(guide_line))?;
        }
    }

    Ok(())
}

/// Renders keyword search results to stdout with matched keywords highlighted (AC-09 / TP-15).
pub fn render_search_results(
    query: &str,
    results: &[DaySearchResult],
    lang: Language,
    theme: ThemeName,
) {
    let mut out = io::stdout();
    let _ = render_search_results_to(&mut out, query, results, lang, theme);
}

pub fn render_search_results_to<W: Write + ?Sized>(
    out: &mut W,
    query: &str,
    results: &[DaySearchResult],
    lang: Language,
    theme: ThemeName,
) -> io::Result<()> {
    let palette = theme.palette();

    if results.is_empty() {
        match lang {
            Language::Ja => writeln!(out, "* 「{}」に一致する記録は見つかりませんでした。", query)?,
            Language::En => writeln!(out, "* No records found matching '{}'.", query)?,
        }
        return Ok(());
    }

    match lang {
        Language::Ja => writeln!(
            out,
            "* 「{}」の検索結果（{}日分ヒット）:",
            query,
            results.len()
        )?,
        Language::En => writeln!(
            out,
            "* Search results for '{}' ({} day(s) matched):",
            query,
            results.len()
        )?,
    }

    for day in results {
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            palette.date_header.paint(&format!(
                "{} ({})",
                day.date.format_header_for(lang),
                day.date.format_ymd()
            ))
        )?;

        if !day.matched_tasks.is_empty() {
            writeln!(
                out,
                "  {}",
                palette.section_heading.paint(lang.todo_section_header())
            )?;
            for tm in &day.matched_tasks {
                let highlighted_task = highlight_keyword(&tm.task.text, query, theme);
                writeln!(
                    out,
                    "  {} {}",
                    tm.task.status.symbol(),
                    highlighted_task
                )?;
                for (s_idx, sub) in tm.task.subtasks.iter().enumerate() {
                    if tm.matched_subtask_indices.contains(&s_idx) || tm.task_matched {
                        let highlighted_sub = highlight_keyword(sub, query, theme);
                        writeln!(out, "    - {}", highlighted_sub)?;
                    }
                }
            }
        }

        if !day.matched_reflection_lines.is_empty() {
            writeln!(
                out,
                "  {}",
                palette
                    .section_heading
                    .paint(lang.reflection_section_header())
            )?;
            for line in &day.matched_reflection_lines {
                writeln!(out, "  {}", highlight_keyword(line, query, theme))?;
            }
        }
    }

    Ok(())
}

/// Highlights all case-insensitive occurrences of `query` in `text` using the theme's `new_badge` Bold color.
pub fn highlight_keyword(text: &str, query: &str, theme: ThemeName) -> String {
    let q = query.trim();
    if q.is_empty() {
        return text.to_string();
    }
    let badge_prefix = theme.palette().new_badge.ansi_prefix();
    let text_lower = text.to_lowercase();
    let q_lower = q.to_lowercase();

    if text.len() == text_lower.len() && q.len() == q_lower.len() {
        let mut out = String::with_capacity(text.len() + 24);
        let mut last_end = 0;
        for (idx, _) in text_lower.match_indices(&q_lower) {
            out.push_str(&text[last_end..idx]);
            out.push_str(&badge_prefix);
            out.push_str(&text[idx..idx + q.len()]);
            out.push_str(ANSI_RESET);
            last_end = idx + q.len();
        }
        out.push_str(&text[last_end..]);
        out
    } else {
        text.replace(q, &format!("{}{}{}", badge_prefix, q, ANSI_RESET))
    }
}
