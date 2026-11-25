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
    let mut in_coach_block = false;
    for (idx, line) in log.reflection.iter().enumerate() {
        let is_last = idx + 1 == log.reflection.len();
        let trimmed = line.trim_start();
        if trimmed.starts_with("> [Coach]") {
            in_coach_block = true;
        } else if !trimmed.starts_with('>') {
            in_coach_block = false;
        }

        if highlight == HighlightTarget::Reflection && is_last {
            writeln!(
                out,
                "{}",
                palette.new_badge.paint(&format!("{}  [NEW]", line))
            )?;
        } else if in_coach_block {
            writeln!(out, "{}", palette.coach_text.paint(line))?;
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
            let mut in_coach_block = false;
            for line in &day.matched_reflection_lines {
                let trimmed = line.trim_start();
                if trimmed.starts_with("> [Coach]") {
                    in_coach_block = true;
                } else if !trimmed.starts_with('>') {
                    in_coach_block = false;
                }
                let base_prefix = if in_coach_block {
                    Some(palette.coach_text.ansi_prefix())
                } else {
                    None
                };
                let highlighted =
                    highlight_keyword_with_base(line, query, theme, base_prefix.as_deref());
                writeln!(out, "  {}", highlighted)?;
            }
        }
    }

    Ok(())
}

/// Highlights all case-insensitive occurrences of `query` in `text` using the theme's `new_badge` Bold color.
pub fn highlight_keyword(text: &str, query: &str, theme: ThemeName) -> String {
    highlight_keyword_with_base(text, query, theme, None)
}

/// Highlights occurrences of `query` in `text` while preserving an optional `base_prefix` styling across text segments.
pub fn highlight_keyword_with_base(
    text: &str,
    query: &str,
    theme: ThemeName,
    base_prefix: Option<&str>,
) -> String {
    let q = query.trim();
    let base = base_prefix.unwrap_or("");
    let has_base = !base.is_empty();

    if q.is_empty() {
        if has_base {
            return format!("{}{}{}", base, text, ANSI_RESET);
        }
        return text.to_string();
    }
    let badge_prefix = theme.palette().new_badge.ansi_prefix();
    let text_lower = text.to_lowercase();
    let q_lower = q.to_lowercase();

    if text.len() == text_lower.len() && q.len() == q_lower.len() {
        let mut out = String::with_capacity(text.len() + 32);
        if has_base {
            out.push_str(base);
        }
        let mut last_end = 0;
        for (idx, _) in text_lower.match_indices(&q_lower) {
            out.push_str(&text[last_end..idx]);
            out.push_str(&badge_prefix);
            out.push_str(&text[idx..idx + q.len()]);
            out.push_str(ANSI_RESET);
            if has_base {
                out.push_str(base);
            }
            last_end = idx + q.len();
        }
        out.push_str(&text[last_end..]);
        if has_base {
            out.push_str(ANSI_RESET);
        }
        out
    } else {
        let mut out = String::new();
        if has_base {
            out.push_str(base);
        }
        let replacement = if has_base {
            format!("{}{}{}{}", badge_prefix, q, ANSI_RESET, base)
        } else {
            format!("{}{}{}", badge_prefix, q, ANSI_RESET)
        };
        out.push_str(&text.replace(q, &replacement));
        if has_base {
            out.push_str(ANSI_RESET);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date::LocalDate;
    use crate::domain::journal::DailyLog;

    #[test]
    fn test_render_coach_quote_block_theme_colors() {
        let mut log = DailyLog::new(LocalDate::new(2026, 10, 17));
        log.header = "### 10/17(土)".to_string();
        log.reflection = vec![
            "普通のふりかえり本文です。".to_string(),
            "> [Coach] ここはコーチの受容文です。".to_string(),
            "> 質問：これはコーチの問いかけ文です？".to_string(),
            "通常のふりかえり本文が再開。".to_string(),
            "> 普通の引用文（コーチではない）。".to_string(),
        ];

        let presets = [
            (ThemeName::TokyoNight, "\x1b[38;2;125;207;255m", "\x1b[38;2;192;202;245m"),
            (ThemeName::Monokai, "\x1b[38;2;174;129;255m", "\x1b[38;2;248;248;242m"),
            (ThemeName::Nord, "\x1b[38;2;143;188;187m", "\x1b[38;2;236;239;244m"),
            (ThemeName::Sepia, "\x1b[38;2;107;112;92m", "\x1b[38;2;250;237;205m"),
        ];

        for (theme, expected_coach_ansi, expected_normal_ansi) in presets {
            let mut buf = Vec::new();
            render_daily_log_to_with_options(
                &mut buf,
                &log,
                HighlightTarget::None,
                false,
                Language::Ja,
                theme,
            )
            .unwrap();
            let rendered = String::from_utf8(buf).unwrap();

            // > [Coach] line should have theme coach_text ANSI color
            let expected_coach_header = format!(
                "{}> [Coach] ここはコーチの受容文です。\x1b[0m",
                expected_coach_ansi
            );
            assert!(
                rendered.contains(&expected_coach_header),
                "Theme {:?} should style `> [Coach]` with coach_text ANSI:\n{}",
                theme,
                rendered
            );

            // Subsequent `> ` line within coach block should also have theme coach_text ANSI color
            let expected_coach_question = format!(
                "{}> 質問：これはコーチの問いかけ文です？\x1b[0m",
                expected_coach_ansi
            );
            assert!(
                rendered.contains(&expected_coach_question),
                "Theme {:?} should style subsequent `>` line with coach_text ANSI:\n{}",
                theme,
                rendered
            );

            // Regular reflection text should have reflection_body ANSI color
            let expected_normal = format!(
                "{}普通のふりかえり本文です。\x1b[0m",
                expected_normal_ansi
            );
            assert!(
                rendered.contains(&expected_normal),
                "Theme {:?} should style normal text with reflection_body ANSI:\n{}",
                theme,
                rendered
            );

            // Unrelated blockquote should NOT have coach_text ANSI color (should have reflection_body)
            let expected_unrelated_quote = format!(
                "{}> 普通の引用文（コーチではない）。\x1b[0m",
                expected_normal_ansi
            );
            assert!(
                rendered.contains(&expected_unrelated_quote),
                "Theme {:?} should NOT style unrelated blockquote with coach_text ANSI:\n{}",
                theme,
                rendered
            );
        }
    }

    #[test]
    fn test_search_results_with_coach_text_highlight() {
        let results = vec![DaySearchResult {
            date: LocalDate::new(2026, 10, 17),
            header: "### 10/17(土)".to_string(),
            matched_tasks: Vec::new(),
            matched_reflection_lines: vec![
                "> [Coach] 環境が快適になって読書が楽しいですね。".to_string(),
                "> 質問：次の一歩は何ですか？".to_string(),
            ],
        }];

        let mut buf = Vec::new();
        render_search_results_to(&mut buf, "快適", &results, Language::Ja, ThemeName::Nord)
            .unwrap();
        let rendered = String::from_utf8(buf).unwrap();

        // Nord coach_text prefix: \x1b[38;2;143;188;187m
        assert!(
            rendered.contains("\x1b[38;2;143;188;187m"),
            "Search result should contain Nord coach_text color prefix:\n{}",
            rendered
        );
        // Nord new_badge prefix: \x1b[1;38;2;163;190;140m
        assert!(
            rendered.contains("\x1b[1;38;2;163;190;140m快適\x1b[0m"),
            "Search result should highlight matched keyword '快適':\n{}",
            rendered
        );
    }
}
