pub mod cli;
pub mod dialect;
pub mod pipeline;
pub mod prompt;

pub use cli::*;
pub use dialect::*;
pub use pipeline::*;
pub use prompt::*;

use serde::Deserialize;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_PATH: &str = "~/.config/yarube/config.toml";
pub const DEFAULT_PLAMO_MODEL_PATH: &str =
    "~/.local/share/yarube/models/plamo-3-translate-0.8b-Q4_K_M.gguf";
pub const FALLBACK_PLAMO_MODEL_PATH: &str =
    "~/.local/share/yarube/models/plamo-3-translate-0.8b.gguf";
pub const DEFAULT_QWEN_MODEL_PATH: &str =
    "~/.local/share/yarube/models/Qwen3-1.7B-Q8_0.gguf";
pub const FALLBACK_QWEN_MODEL_PATH: &str = "~/.local/share/yarube/models/model.gguf";
pub const DEFAULT_QWEN_TOKENIZER_PATH: &str = "~/.local/share/yarube/models/tokenizer.json";
pub const DEFAULT_DIALECT: &str = "hokkaido_donan";
pub const DEFAULT_DIALECTS_DIR: &str = "~/.config/yarube/dialects";
pub const DEFAULT_USER_PROMPT_EN: &str = "~/.config/yarube/coach_ja_prompt.toml";

pub const DEFAULT_STEP1_MAX_TOKENS: usize = 256;
pub const DEFAULT_STEP2_MAX_TOKENS: usize = 128;
pub const DEFAULT_STEP3_MAX_TOKENS: usize = 200;
pub const DEFAULT_TEMPERATURE: f64 = 0.3;
pub const DEFAULT_TOP_K: usize = 10;
pub const DEFAULT_TOP_P: f64 = 0.8;
pub const DEFAULT_REPEAT_PENALTY: f32 = 1.1;
pub const DEFAULT_REPEAT_LAST_N: usize = 64;
pub const DEFAULT_COACH_HEX: &str = "#7dcfff";
pub const DEFAULT_COACH_RGB: (u8, u8, u8) = (0x7d, 0xcf, 0xff);

pub const DEFAULT_EN_PROMPT_TOML: &str = include_str!("../prompts/default_en.toml");
pub const BUILTIN_DIALECT_STANDARD_POLITE: &str =
    include_str!("../dialects/standard_polite.json");
pub const BUILTIN_DIALECT_HOKKAIDO_DONAN: &str =
    include_str!("../dialects/hokkaido_donan.json");
pub const BUILTIN_DIALECT_KANSAI: &str = include_str!("../dialects/kansai.json");

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct OnReflectPayload {
    #[allow(dead_code)]
    #[serde(default)]
    pub event: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub date: String,
    pub log_path: String,
    pub added_text: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct RootConfig {
    #[serde(default)]
    pub plugins: PluginsSection,
}

#[derive(Debug, Deserialize, Default)]
pub struct PluginsSection {
    #[serde(default, rename = "coach-ja", alias = "coach_ja")]
    pub coach_ja: CoachJaConfig,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct CoachJaConfig {
    pub enabled: bool,
    pub plamo_model_path: String,
    #[serde(alias = "model_path")]
    pub qwen_model_path: String,
    #[serde(alias = "tokenizer_path")]
    pub qwen_tokenizer_path: String,
    pub dialect: String,
    pub dialects_dir: String,
    pub prompt_path: Option<String>,
    pub step1_max_tokens: usize,
    #[serde(alias = "max_tokens")]
    pub step2_max_tokens: usize,
    pub step3_max_tokens: usize,
    pub temperature: f64,
    pub top_k: usize,
    pub top_p: f64,
    pub repeat_penalty: f32,
    pub repeat_last_n: usize,
    /// When `false` (default), PLaMo 3 and Qwen3 model weights are explicitly `drop`ped after each step to minimize peak RAM.
    /// When `true`, PLaMo 3 loaded in Step 1 is kept in memory, its KV cache is cleared (`clear_kv_cache`), and reused in Step 3.
    pub keep_plamo_in_memory: bool,
}

impl Default for CoachJaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            plamo_model_path: DEFAULT_PLAMO_MODEL_PATH.to_string(),
            qwen_model_path: DEFAULT_QWEN_MODEL_PATH.to_string(),
            qwen_tokenizer_path: DEFAULT_QWEN_TOKENIZER_PATH.to_string(),
            dialect: DEFAULT_DIALECT.to_string(),
            dialects_dir: DEFAULT_DIALECTS_DIR.to_string(),
            prompt_path: None,
            step1_max_tokens: DEFAULT_STEP1_MAX_TOKENS,
            step2_max_tokens: DEFAULT_STEP2_MAX_TOKENS,
            step3_max_tokens: DEFAULT_STEP3_MAX_TOKENS,
            temperature: DEFAULT_TEMPERATURE,
            top_k: DEFAULT_TOP_K,
            top_p: DEFAULT_TOP_P,
            repeat_penalty: DEFAULT_REPEAT_PENALTY,
            repeat_last_n: DEFAULT_REPEAT_LAST_N,
            keep_plamo_in_memory: false,
        }
    }
}

pub fn home_dir() -> Option<PathBuf> {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

pub fn expand_tilde(path_str: &str) -> PathBuf {
    let trimmed = path_str.trim();
    if trimmed == "~" || trimmed.starts_with("~/") || trimmed.starts_with("~\\") {
        if let Some(home) = home_dir() {
            if trimmed == "~" {
                return home;
            }
            return home.join(&trimmed[2..]);
        }
    }
    PathBuf::from(trimmed)
}

pub fn resolve_active_config_path() -> PathBuf {
    if let Ok(env_cfg) = env::var("YARUBE_CONFIG_PATH") {
        let trimmed = env_cfg.trim();
        if !trimmed.is_empty() {
            let expanded = expand_tilde(trimmed);
            if expanded.is_file() {
                return expanded;
            }
        }
    }
    for candidate in [
        "~/.config/yarube/config.toml",
        "~/.yarube/config.toml",
        "~/.yarube/config",
    ] {
        let expanded = expand_tilde(candidate);
        if expanded.is_file() {
            return expanded;
        }
    }
    if let Ok(env_cfg) = env::var("YARUBE_CONFIG_PATH") {
        let trimmed = env_cfg.trim();
        if !trimmed.is_empty() {
            return expand_tilde(trimmed);
        }
    }
    expand_tilde(DEFAULT_CONFIG_PATH)
}

pub fn load_coach_ja_config(config_path: &Path) -> CoachJaConfig {
    let mut candidates = vec![config_path.to_path_buf()];
    for extra in [
        "~/.config/yarube/config.toml",
        "~/.yarube/config.toml",
        "~/.yarube/config",
    ] {
        let p = expand_tilde(extra);
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if content.contains("[plugins.coach-ja]") || content.contains("[plugins.coach_ja]") {
                if let Ok(root) = toml::from_str::<RootConfig>(&content) {
                    return root.plugins.coach_ja;
                }
            }
        }
    }

    let Ok(content) = std::fs::read_to_string(config_path) else {
        return CoachJaConfig::default();
    };
    match toml::from_str::<RootConfig>(&content) {
        Ok(root) => root.plugins.coach_ja,
        Err(_) => CoachJaConfig::default(),
    }
}

pub fn parse_hex_rgb(hex_str: &str) -> (u8, u8, u8) {
    let trimmed = hex_str.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() == 6 && hex.is_ascii() {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return (r, g, b);
        }
    }
    DEFAULT_COACH_RGB
}

pub fn extract_stream_chunk(decoded: &str, printed_len: &mut usize) -> Option<String> {
    if decoded.ends_with('\u{FFFD}') || decoded.len() <= *printed_len {
        return None;
    }
    let diff = &decoded[*printed_len..];
    *printed_len = decoded.len();
    Some(diff.replace('\n', "\n> "))
}

pub fn format_coach_markdown_block(generated_text: &str) -> Option<String> {
    let trimmed = generated_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (idx, line) in trimmed.lines().enumerate() {
        let clean_line = line.trim_end_matches('\r');
        if idx == 0 {
            out.push_str("> [Coach] ");
            out.push_str(clean_line);
            out.push('\n');
        } else {
            out.push_str("> ");
            out.push_str(clean_line);
            out.push('\n');
        }
    }
    Some(out)
}

pub fn append_coach_to_log(log_path: &Path, generated_text: &str) -> io::Result<()> {
    let Some(block) = format_coach_markdown_block(generated_text) else {
        return Ok(());
    };

    let needs_leading_newline = if let Ok(mut existing) = File::open(log_path) {
        let mut buf = Vec::new();
        let _ = existing.read_to_end(&mut buf);
        !buf.is_empty() && !buf.ends_with(b"\n")
    } else {
        false
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    if needs_leading_newline {
        file.write_all(b"\n")?;
    }
    file.write_all(block.as_bytes())?;
    file.flush()
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    match parse_cli_args(&args) {
        CliCommand::Setup(sub_args) => {
            std::process::exit(run_setup(&sub_args));
        }
        CliCommand::Status => {
            std::process::exit(run_status());
        }
        CliCommand::Dialect(target) => {
            std::process::exit(run_dialect(target.as_deref()));
        }
        CliCommand::Help => {
            println!("[yarube-coach-ja]");
            println!("Pure Rust 2-stage reflection coach (PLaMo 3 Translate 0.8B x Qwen3-1.7B + JSON Dialect)");
            println!("Usage:");
            println!("  yarube coach-ja status");
            println!("  yarube coach-ja setup [--no-download]");
            println!("  yarube coach-ja dialect [standard_polite|hokkaido_donan|kansai|<path.json>]");
            std::process::exit(0);
        }
        CliCommand::Inference => {
            if io::stdin().is_terminal() {
                std::process::exit(run_status());
            }
            let _ = run_coach_ja_pipeline();
            Ok(())
        }
    }
}
