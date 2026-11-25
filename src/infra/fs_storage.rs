use crate::domain::config::{ensure_abs_path, expand_tilde, home_dir, resolve_abs_path, AppConfig};
use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::domain::journal::{DailyLog, Task, TaskStatus};
use crate::ports::{ConfigRepository, JournalRepository, LoadResult};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct FsStorage {
    base_dir: PathBuf,
    explicit_dir: bool,
}

impl FsStorage {
    pub fn from_env() -> io::Result<Self> {
        for key in ["YARUBE_DIR", "BUJO_DIR"] {
            if let Ok(dir) = env::var(key) {
                if !dir.trim().is_empty() {
                    return Ok(Self {
                        base_dir: resolve_abs_path(&dir),
                        explicit_dir: true,
                    });
                }
            }
        }
        Ok(Self {
            base_dir: Self::default_dir()?,
            explicit_dir: false,
        })
    }

    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir: ensure_abs_path(base_dir),
            explicit_dir: true,
        }
    }

    fn default_dir() -> io::Result<PathBuf> {
        let home = home_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "HOME 環境変数が見つかりません。YARUBE_DIR を設定してください。",
            )
        })?;
        Ok(ensure_abs_path(home.join(".yarube")))
    }

    pub fn file_path(&self, date: LocalDate) -> PathBuf {
        self.base_dir.join(format!("{}.md", date.format_ymd()))
    }

    fn candidate_config_paths(&self) -> Vec<PathBuf> {
        let mut candidates = vec![
            self.base_dir.join("config.toml"),
            self.base_dir.join("config"),
        ];
        if !self.explicit_dir {
            if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
                if !xdg.trim().is_empty() {
                    let xdg_dir = expand_tilde(&xdg).join("yarube");
                    candidates.push(xdg_dir.join("config.toml"));
                    candidates.push(xdg_dir.join("config"));
                }
            }
            if let Some(home) = home_dir() {
                let home_cfg_dir = home.join(".config").join("yarube");
                candidates.push(home_cfg_dir.join("config.toml"));
                candidates.push(home_cfg_dir.join("config"));
            }
        }
        candidates
    }

    pub fn config_path(&self) -> PathBuf {
        for candidate in self.candidate_config_paths() {
            if candidate.exists() {
                return candidate;
            }
        }
        self.base_dir.join("config.toml")
    }

    pub fn abs_file_path(&self, date: LocalDate) -> PathBuf {
        ensure_abs_path(self.file_path(date))
    }

    pub fn abs_config_path(&self) -> PathBuf {
        ensure_abs_path(self.config_path())
    }

    pub fn active_language(&self) -> Language {
        self.load_config()
            .unwrap_or_default()
            .resolve_language()
    }
}

impl JournalRepository for FsStorage {
    fn load_or_init(&self, date: LocalDate) -> io::Result<LoadResult> {
        fs::create_dir_all(&self.base_dir)?;
        let path = self.file_path(date);
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let log = DailyLog::parse_markdown(date, &content);
            Ok(LoadResult {
                log,
                newly_created: false,
            })
        } else {
            let lang = self.active_language();
            let log = DailyLog::new_with_lang(date, lang);
            self.save(&log)?;
            Ok(LoadResult {
                log,
                newly_created: true,
            })
        }
    }

    fn load_existing(&self, date: LocalDate) -> io::Result<Option<DailyLog>> {
        let path = self.file_path(date);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        Ok(Some(DailyLog::parse_markdown(date, &content)))
    }

    fn save(&self, log: &DailyLog) -> io::Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        let path = self.file_path(log.date);
        let tmp_path = self.base_dir.join(format!(
            ".{}.md.tmp.{}",
            log.date.format_ymd(),
            std::process::id()
        ));
        let lang = self.active_language();
        let markdown = log.to_markdown_with_lang(lang);
        fs::write(&tmp_path, markdown)?;
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    fn migrate_task_to_next_day(&self, current_date: LocalDate, task: &Task) -> io::Result<()> {
        let next_date = current_date.next_day();
        let mut next_res = self.load_or_init(next_date)?;

        let already_exists = next_res.log.tasks.iter().any(|t| {
            t.status == TaskStatus::Todo && t.text == task.text && t.subtasks == task.subtasks
        });

        if !already_exists {
            next_res.log.tasks.push(Task {
                status: TaskStatus::Todo,
                text: task.text.clone(),
                subtasks: task.subtasks.clone(),
            });
            self.save(&next_res.log)?;
        }
        Ok(())
    }

    fn list_recorded_dates(&self) -> io::Result<Vec<LocalDate>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut dates = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(date) = LocalDate::parse_ymd(stem) {
                        dates.push(date);
                    }
                }
            }
        }
        dates.sort();
        dates.dedup();
        Ok(dates)
    }

    fn load_all_logs(&self) -> io::Result<Vec<DailyLog>> {
        let dates = self.list_recorded_dates()?;
        let mut logs = Vec::with_capacity(dates.len());
        for date in dates {
            if let Some(log) = self.load_existing(date)? {
                logs.push(log);
            }
        }
        Ok(logs)
    }

    fn log_file_path(&self, date: LocalDate) -> PathBuf {
        self.abs_file_path(date)
    }
}

impl ConfigRepository for FsStorage {
    fn load_config(&self) -> io::Result<AppConfig> {
        for candidate in self.candidate_config_paths() {
            if candidate.exists() {
                let content = fs::read_to_string(&candidate)?;
                return Ok(AppConfig::parse(&content));
            }
        }
        Ok(AppConfig::default())
    }

    fn save_config(&self, config: &AppConfig) -> io::Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        let content = config.to_config_string();
        for target_name in ["config.toml", "config"] {
            let path = self.base_dir.join(target_name);
            let tmp_path = self
                .base_dir
                .join(format!(".{}.tmp.{}", target_name, std::process::id()));
            fs::write(&tmp_path, &content)?;
            fs::rename(&tmp_path, &path)?;
        }
        // If an external XDG config (`~/.config/yarube/config.toml`) already exists and we are not in explicit_dir mode, keep it in sync as well.
        if !self.explicit_dir {
            for candidate in self.candidate_config_paths().into_iter().skip(2) {
                if candidate.exists() {
                    if let Some(parent) = candidate.parent() {
                        let tmp_path = parent.join(format!(".config.tmp.{}", std::process::id()));
                        if fs::write(&tmp_path, &content).is_ok() {
                            let _ = fs::rename(&tmp_path, &candidate);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn config_file_path(&self) -> PathBuf {
        self.abs_config_path()
    }
}
