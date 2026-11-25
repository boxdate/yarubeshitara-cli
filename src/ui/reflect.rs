use crate::domain::journal::{DailyLog, HighlightTarget};
use crate::infra::plugin_runner::ProcessPluginRunner;
use crate::ports::{
    ConfigRepository, EditorLauncher, JournalRepository, PluginEnvContext, PluginRunner,
};
use crate::ui::renderer::render_daily_log_with_options;
use std::io::{self, Write};

/// Appends reflection text either directly from CLI args or by launching the resolved editor (EARS-11, EARS-12).
/// Guarantees persistence before firing `on_reflect` lifecycle hooks (EARS-26, EARS-27, EARS-28, EARS-30).
pub fn run_reflect<R, C, E>(
    journal_repo: &R,
    config_repo: &C,
    editor_launcher: &E,
    log: DailyLog,
    inline_text: Option<String>,
) -> io::Result<()>
where
    R: JournalRepository,
    C: ConfigRepository,
    E: EditorLauncher,
{
    let plugin_runner = ProcessPluginRunner;
    run_reflect_with_plugins(
        journal_repo,
        config_repo,
        editor_launcher,
        &plugin_runner,
        log,
        inline_text,
    )
}

pub fn run_reflect_with_plugins<R, C, E, P>(
    journal_repo: &R,
    config_repo: &C,
    editor_launcher: &E,
    plugin_runner: &P,
    mut log: DailyLog,
    inline_text: Option<String>,
) -> io::Result<()>
where
    R: JournalRepository,
    C: ConfigRepository,
    E: EditorLauncher,
    P: PluginRunner,
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

    let trimmed = text.trim();
    if trimmed.is_empty() {
        println!("{}", lang.reflect_empty_skipped());
        return Ok(());
    }

    // 1. Persist and flush reflection to Markdown file FIRST (EARS-26)
    log.append_reflection(&text);
    journal_repo.save(&log)?;

    // 2. Redraw today's journal to stdout and flush BEFORE launching hooks
    render_daily_log_with_options(&log, HighlightTarget::Reflection, false, lang, theme);
    let _ = io::stdout().flush();

    // 3. Fire `on_reflect` lifecycle hooks and wait (`child.wait()`) AFTER save & redraw (EARS-26, EARS-27, EARS-28, EARS-30)
    let env_ctx = PluginEnvContext {
        lang,
        log_path: journal_repo.log_file_path(log.date),
        config_path: config_repo.config_file_path(),
        theme,
    };
    plugin_runner.run_on_reflect_hooks(&cfg, &env_ctx, log.date, trimmed);

    Ok(())
}
