use crate::domain::config::{ensure_abs_path, AppConfig};
use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::ports::{PluginEnvContext, PluginRunner};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct ProcessPluginRunner;

pub fn is_debug_enabled() -> bool {
    matches!(
        env::var("YARUBE_DEBUG").ok().as_deref().map(str::trim),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

impl ProcessPluginRunner {
    /// Resolves `yarube-<name>` (or `yarube-<name>.exe` on Windows) in priority order (EARS-29):
    /// 1. `config.plugins.dir` (default: `~/.local/share/yarube/plugins/`)
    /// 2. Environment variable `PATH`
    pub fn resolve_plugin_binary(config: &AppConfig, plugin_name: &str) -> Option<PathBuf> {
        let debug = is_debug_enabled();
        let name = plugin_name.trim();
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.chars().any(char::is_whitespace)
        {
            if debug {
                eprintln!("[debug] invalid plugin name skipped: '{}'", plugin_name);
            }
            return None;
        }

        let base_bin_name = format!("yarube-{}", name);

        // 1. Check config.plugins.dir (resolved to absolute path with ~ expansion)
        let plugins_dir = config.plugins.resolved_dir();
        let primary_candidate = plugins_dir.join(&base_bin_name);
        if debug {
            let exists = primary_candidate.exists();
            let exec = is_executable_file(&primary_candidate);
            eprintln!(
                "[debug] checking plugin: {} (exists: {}, executable: {})",
                primary_candidate.display(),
                exists,
                exec
            );
        }
        if let Some(found) = find_executable_in_dir(&plugins_dir, &base_bin_name) {
            let abs_found = ensure_abs_path(found);
            if debug {
                eprintln!(
                    "[debug] resolved plugin in plugins.dir: {}",
                    abs_found.display()
                );
            }
            return Some(abs_found);
        }

        // 2. Check environment variable PATH
        if let Some(path_os) = env::var_os("PATH") {
            for dir in env::split_paths(&path_os) {
                if let Some(found) = find_executable_in_dir(&dir, &base_bin_name) {
                    let abs_found = ensure_abs_path(found);
                    if debug {
                        eprintln!(
                            "[debug] resolved plugin in PATH: {}",
                            abs_found.display()
                        );
                    }
                    return Some(abs_found);
                }
            }
        }

        if debug {
            eprintln!(
                "[debug] plugin '{}' not found in plugins.dir ({}) or PATH",
                base_bin_name,
                plugins_dir.display()
            );
        }
        None
    }

    /// Injects common `YARUBE_*` environment variables into a child `Command` (EARS-30).
    pub fn inject_common_env(cmd: &mut Command, env_ctx: &PluginEnvContext) {
        let abs_log_path = ensure_abs_path(env_ctx.log_path.clone());
        let abs_config_path = ensure_abs_path(env_ctx.config_path.clone());

        cmd.env("YARUBE_VERSION", env!("CARGO_PKG_VERSION"));
        cmd.env("YARUBE_LANG", env_ctx.lang.code());
        cmd.env("YARUBE_LOG_PATH", &abs_log_path);
        cmd.env("YARUBE_CONFIG_PATH", &abs_config_path);
        cmd.env("YARUBE_THEME_NAME", env_ctx.theme.as_str());
        cmd.env(
            "YARUBE_COLOR_COACH",
            env_ctx.theme.palette().coach_text.hex,
        );
    }

    /// Executes an external subcommand `yarube-<name> [args...]` with `Stdio::inherit()` (US-09 / AC-17 / EARS-29).
    pub fn execute_subcommand(
        bin_path: &Path,
        args: &[String],
        env_ctx: &PluginEnvContext,
    ) -> io::Result<i32> {
        let abs_bin = ensure_abs_path(bin_path.to_path_buf());
        if is_debug_enabled() {
            eprintln!(
                "[debug] executing external subcommand: {} {:?}",
                abs_bin.display(),
                args
            );
        }
        let mut cmd = Command::new(&abs_bin);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        Self::inject_common_env(&mut cmd, env_ctx);

        let status = cmd.status()?;
        Ok(status.code().unwrap_or(1))
    }

    /// Formats the single-line JSON payload for `on_reflect` lifecycle hooks (Plugin Interface Specification §3).
    pub fn format_on_reflect_json(date: LocalDate, log_path: &Path, added_text: &str) -> String {
        let date_str = date.format_ymd();
        let abs_log = ensure_abs_path(log_path.to_path_buf());
        let path_str = abs_log.to_string_lossy();
        format!(
            "{{\"event\":\"on_reflect\",\"date\":\"{}\",\"log_path\":\"{}\",\"added_text\":\"{}\"}}",
            escape_json_str(&date_str),
            escape_json_str(&path_str),
            escape_json_str(added_text),
        )
    }
}

impl PluginRunner for ProcessPluginRunner {
    fn run_on_reflect_hooks(
        &self,
        config: &AppConfig,
        env_ctx: &PluginEnvContext,
        date: LocalDate,
        added_text: &str,
    ) {
        let debug = is_debug_enabled();
        let abs_log_path = ensure_abs_path(env_ctx.log_path.clone());
        let json_line = format!(
            "{}\n",
            Self::format_on_reflect_json(date, &abs_log_path, added_text)
        );

        if debug {
            eprintln!(
                "[debug] firing on_reflect hooks: {:?} (plugins.dir: {}, config_path: {}, log_path: {})",
                config.plugins.on_reflect,
                config.plugins.resolved_dir().display(),
                ensure_abs_path(env_ctx.config_path.clone()).display(),
                abs_log_path.display()
            );
        }

        for plugin_name in &config.plugins.on_reflect {
            let trimmed = plugin_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Check [plugins.<name>] enabled = false (BDD 08-3 / EARS-28)
            if !config.plugins.is_plugin_enabled(trimmed) {
                if debug {
                    eprintln!("[debug] plugin '{}' is disabled in config; skipping", trimmed);
                }
                continue;
            }

            // Search for `yarube-<name>`. If not found, silently skip and succeed (BDD 08-1 / EARS-27).
            let Some(bin_path) = Self::resolve_plugin_binary(config, trimmed) else {
                continue;
            };

            if debug {
                eprintln!("[debug] spawning hook plugin: {}", bin_path.display());
            }

            let mut cmd = Command::new(&bin_path);
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            Self::inject_common_env(&mut cmd, env_ctx);

            match cmd.spawn() {
                Ok(mut child) => {
                    if let Some(mut stdin) = child.stdin.take() {
                        if let Err(err) = stdin.write_all(json_line.as_bytes()) {
                            if debug {
                                eprintln!(
                                    "[debug] failed writing JSON to stdin of {}: {}",
                                    bin_path.display(),
                                    err
                                );
                            }
                        }
                        if let Err(err) = stdin.flush() {
                            if debug {
                                eprintln!(
                                    "[debug] failed flushing stdin of {}: {}",
                                    bin_path.display(),
                                    err
                                );
                            }
                        }
                        drop(stdin);
                    }
                    match child.wait() {
                        Ok(status) => {
                            let _ = io::stdout().flush();
                            let _ = io::stderr().flush();
                            if debug {
                                eprintln!(
                                    "[debug] hook plugin {} finished with status: {}",
                                    bin_path.display(),
                                    status
                                );
                            }
                            if !status.success() {
                                print_plugin_warning(env_ctx.lang, trimmed, status.code());
                            }
                        }
                        Err(err) => {
                            if debug {
                                eprintln!(
                                    "[debug] error waiting for hook plugin {}: {}",
                                    bin_path.display(),
                                    err
                                );
                            }
                            print_plugin_warning(env_ctx.lang, trimmed, None);
                        }
                    }
                }
                Err(err) => {
                    if debug {
                        eprintln!(
                            "[debug] failed to spawn hook plugin {}: {}",
                            bin_path.display(),
                            err
                        );
                    }
                    // Process isolation on spawn failure (EARS-27)
                    print_plugin_warning(env_ctx.lang, trimmed, None);
                }
            }
        }
    }
}

fn print_plugin_warning(lang: Language, plugin_name: &str, code: Option<i32>) {
    match (lang, code) {
        (Language::Ja, Some(c)) => {
            eprintln!(
                "[!] 警告: プラグイン 'yarube-{}' が異常終了しました (exit code: {})。ふりかえりの保存は完了しています。",
                plugin_name, c
            );
        }
        (Language::Ja, None) => {
            eprintln!(
                "[!] 警告: プラグイン 'yarube-{}' の実行に失敗しました。ふりかえりの保存は完了しています。",
                plugin_name
            );
        }
        (Language::En, Some(c)) => {
            eprintln!(
                "[!] Warning: Plugin 'yarube-{}' exited with non-zero status (exit code: {}). Reflection has been safely saved.",
                plugin_name, c
            );
        }
        (Language::En, None) => {
            eprintln!(
                "[!] Warning: Failed to execute plugin 'yarube-{}'. Reflection has been safely saved.",
                plugin_name
            );
        }
    }
}

fn find_executable_in_dir(dir: &Path, base_bin_name: &str) -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        for ext in [".exe", ".cmd", ".bat", ""] {
            let candidate = dir.join(format!("{}{}", base_bin_name, ext));
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
        None
    } else {
        let candidate = dir.join(base_bin_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
        let exe_candidate = dir.join(format!("{}.exe", base_bin_name));
        if is_executable_file(&exe_candidate) {
            return Some(exe_candidate);
        }
        None
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_on_reflect_json() {
        let date = LocalDate::new(2026, 10, 17);
        let path = Path::new("/home/user/.local/share/yarube/logs/2026-10-17.md");
        let json = ProcessPluginRunner::format_on_reflect_json(
            date,
            path,
            "テストする気が起きない\n\"引用\"と\\バックスラッシュ",
        );
        assert_eq!(
            json,
            r#"{"event":"on_reflect","date":"2026-10-17","log_path":"/home/user/.local/share/yarube/logs/2026-10-17.md","added_text":"テストする気が起きない\n\"引用\"と\\バックスラッシュ"}"#
        );
    }
}
