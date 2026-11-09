pub mod cli;
pub mod model;
pub mod prompt;

pub use cli::*;
pub use model::*;
pub use prompt::*;

use serde::Deserialize;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_PATH: &str = "~/.config/yarube/config.toml";
pub const DEFAULT_MODEL_PATH: &str = "~/.local/share/yarube/models/Qwen3-1.7B-Q8_0.gguf";
pub const FALLBACK_MODEL_PATH: &str = "~/.local/share/yarube/models/model.gguf";
pub const DEFAULT_TOKENIZER_PATH: &str = "~/.local/share/yarube/models/tokenizer.json";
pub const DEFAULT_USER_PROMPT_JA: &str = "~/.config/yarube/coach_prompt.toml";
pub const DEFAULT_USER_PROMPT_EN: &str = "~/.config/yarube/coach_prompt_en.toml";
pub const DEFAULT_MAX_TOKENS: usize = 100;
pub const DEFAULT_TEMPERATURE: f64 = 0.3;
pub const DEFAULT_TOP_K: usize = 10;
pub const DEFAULT_TOP_P: f64 = 0.8;
pub const DEFAULT_COACH_HEX: &str = "#7dcfff";
pub const DEFAULT_COACH_RGB: (u8, u8, u8) = (0x7d, 0xcf, 0xff);

pub const DEFAULT_JA_PROMPT_TOML: &str = include_str!("../prompts/default_ja.toml");
pub const DEFAULT_EN_PROMPT_TOML: &str = include_str!("../prompts/default_en.toml");

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
    #[serde(default)]
    pub coach: CoachConfig,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct CoachConfig {
    pub enabled: bool,
    pub model_path: String,
    pub tokenizer_path: String,
    pub prompt_path: Option<String>,
    pub max_tokens: usize,
    pub temperature: f64,
    pub top_k: usize,
    pub top_p: f64,
}

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model_path: DEFAULT_MODEL_PATH.to_string(),
            tokenizer_path: DEFAULT_TOKENIZER_PATH.to_string(),
            prompt_path: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            temperature: DEFAULT_TEMPERATURE,
            top_k: DEFAULT_TOP_K,
            top_p: DEFAULT_TOP_P,
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

pub fn load_coach_config(config_path: &Path) -> CoachConfig {
    let Ok(content) = std::fs::read_to_string(config_path) else {
        return CoachConfig::default();
    };
    match toml::from_str::<RootConfig>(&content) {
        Ok(root) => root.plugins.coach,
        Err(_) => CoachConfig::default(),
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

pub fn is_english_lang(lang_val: Option<&str>) -> bool {
    let Some(raw) = lang_val else {
        return false;
    };
    let lower = raw.trim().to_ascii_lowercase();
    lower == "en" || lower.starts_with("en_") || lower.starts_with("en-")
}

pub fn extract_stream_chunk<'a>(decoded: &'a str, printed_len: &mut usize) -> Option<String> {
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
        CliCommand::Help => {
            println!("[yarube-coach]");
            println!("Usage:");
            println!("  yarube coach status");
            println!("  yarube coach setup [--no-download]");
            std::process::exit(0);
        }
        CliCommand::Inference => {
            if io::stdin().is_terminal() {
                std::process::exit(run_status());
            }
            let _ = run_coach();
            Ok(())
        }
    }
}
