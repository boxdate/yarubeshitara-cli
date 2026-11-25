use crate::domain::config::AppConfig;
use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::domain::journal::{DailyLog, Task};
use crate::domain::theme::ThemeName;
use std::io;
use std::path::PathBuf;

pub struct LoadResult {
    pub log: DailyLog,
    pub newly_created: bool,
}

/// Abstract port for persisting and querying daily logs (Loose Coupling & Testability).
pub trait JournalRepository {
    /// Loads the daily log for `date`, or initializes a new template file if it does not exist.
    fn load_or_init(&self, date: LocalDate) -> io::Result<LoadResult>;

    /// Loads the daily log for `date` only if it already exists on disk (does not create a new file).
    fn load_existing(&self, date: LocalDate) -> io::Result<Option<DailyLog>>;

    /// Atomically saves the given `DailyLog`.
    fn save(&self, log: &DailyLog) -> io::Result<()>;

    /// Migrates `task` (including subtasks) to `current_date.next_day()` as an incomplete (`•`) task.
    fn migrate_task_to_next_day(&self, current_date: LocalDate, task: &Task) -> io::Result<()>;

    /// Lists all recorded dates (`YYYY-MM-DD.md`) sorted in ascending chronological order.
    fn list_recorded_dates(&self) -> io::Result<Vec<LocalDate>>;

    /// Loads all recorded `DailyLog` entries sorted in ascending chronological order.
    fn load_all_logs(&self) -> io::Result<Vec<DailyLog>>;

    /// Returns the absolute path to the daily log file for `date`.
    fn log_file_path(&self, date: LocalDate) -> PathBuf {
        PathBuf::from(format!("{}.md", date.format_ymd()))
    }
}

/// Abstract port for loading and saving application configuration (`~/.bujo/config`).
pub trait ConfigRepository {
    fn load_config(&self) -> io::Result<AppConfig>;
    fn save_config(&self, config: &AppConfig) -> io::Result<()>;

    /// Returns the absolute path to the configuration file.
    fn config_file_path(&self) -> PathBuf {
        PathBuf::from("config.toml")
    }
}

/// Abstract port for launching an external text editor to capture reflection text.
pub trait EditorLauncher {
    fn edit_reflection(&self, editor_cmd: &str, date: LocalDate, header: &str) -> io::Result<String>;
}

/// Context passed via environment variables (`YARUBE_*`) and stdin JSON to plugins (EARS-30).
#[derive(Debug, Clone)]
pub struct PluginEnvContext {
    pub lang: Language,
    pub log_path: PathBuf,
    pub config_path: PathBuf,
    pub theme: ThemeName,
}

/// Abstract port for running lifecycle hooks (`on_reflect`) and dynamic subcommands (`yarube-<name>`).
pub trait PluginRunner {
    fn run_on_reflect_hooks(
        &self,
        config: &AppConfig,
        env_ctx: &PluginEnvContext,
        date: LocalDate,
        added_text: &str,
    );
}
