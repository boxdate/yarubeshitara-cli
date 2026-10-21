use crate::domain::journal::{DailyLog, HighlightTarget};
use crate::ports::{ConfigRepository, EditorLauncher, JournalRepository};
use crate::ui::renderer::render_daily_log_with_options;
use std::io;

/// Appends reflection text either directly from CLI args or by launching the resolved editor (EARS-11, EARS-12).
pub fn run_reflect<R, C, E>(
    journal_repo: &R,
    config_repo: &C,
    editor_launcher: &E,
    mut log: DailyLog,
    inline_text: Option<String>,
) -> io::Result<()>
where
    R: JournalRepository,
    C: ConfigRepository,
    E: EditorLauncher,
{
    let cfg = config_repo.load_config()?;
    let lang = cfg.resolve_language();
    let theme = cfg.resolve_theme().theme;

    let text = match inline_text {
        Some(t) => t,
        None => {
            let editor_cmd = cfg.resolve_editor();
            editor_launcher.edit_reflection(&editor_cmd, log.date, &log.header)?
        }
    };

    if text.trim().is_empty() {
        println!("{}", lang.reflect_empty_skipped());
        return Ok(());
    }

    log.append_reflection(&text);
    journal_repo.save(&log)?;
    render_daily_log_with_options(&log, HighlightTarget::Reflection, false, lang, theme);
    Ok(())
}
