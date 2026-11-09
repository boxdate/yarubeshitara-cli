use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights;
use serde::Deserialize;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;

mod cli;
use cli::*;

pub const DEFAULT_CONFIG_PATH: &str = "~/.config/yarube/config.toml";
pub const DEFAULT_MODEL_PATH: &str = "~/.local/share/yarube/models/Qwen3-1.7B-Q8_0.gguf";
pub const FALLBACK_MODEL_PATH: &str = "~/.local/share/yarube/models/model.gguf";
pub const DEFAULT_TOKENIZER_PATH: &str = "~/.local/share/yarube/models/tokenizer.json";
pub const DEFAULT_USER_PROMPT_JA: &str = "~/.config/yarube/coach_prompt.toml";
pub const DEFAULT_USER_PROMPT_EN: &str = "~/.config/yarube/coach_prompt_en.toml";
const DEFAULT_MAX_TOKENS: usize = 100;
const DEFAULT_TEMPERATURE: f64 = 0.3;
const DEFAULT_TOP_K: usize = 10;
const DEFAULT_TOP_P: f64 = 0.8;
const DEFAULT_COACH_HEX: &str = "#7dcfff";
const DEFAULT_COACH_RGB: (u8, u8, u8) = (0x7d, 0xcf, 0xff);

pub const DEFAULT_JA_PROMPT_TOML: &str = include_str!("../prompts/default_ja.toml");
pub const DEFAULT_EN_PROMPT_TOML: &str = include_str!("../prompts/default_en.toml");

#[derive(Debug, Deserialize)]
struct OnReflectPayload {
    #[allow(dead_code)]
    #[serde(default)]
    event: String,
    #[allow(dead_code)]
    #[serde(default)]
    date: String,
    log_path: String,
    added_text: String,
}

#[derive(Debug, Deserialize, Default)]
struct RootConfig {
    #[serde(default)]
    plugins: PluginsSection,
}

#[derive(Debug, Deserialize, Default)]
struct PluginsSection {
    #[serde(default)]
    coach: CoachConfig,
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

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PromptTemplate {
    pub system: String,
    #[serde(default)]
    pub examples: Vec<ExamplePair>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ExamplePair {
    pub user: String,
    pub assistant: String,
}

impl PromptTemplate {
    pub fn render(&self, added_text: &str) -> String {
        let sys_trimmed = self.system.trim();
        let sys_with_no_think = if sys_trimmed.ends_with("/no_think") {
            sys_trimmed.to_string()
        } else {
            format!("{} /no_think", sys_trimmed)
        };

        let mut out = format!("<|im_start|>system\n{}<|im_end|>\n", sys_with_no_think);
        for ex in &self.examples {
            out.push_str(&format!(
                "<|im_start|>user\n{}<|im_end|>\n\
                 <|im_start|>assistant\n<think>\n\n</think>\n\n{}<|im_end|>\n",
                ex.user.trim(),
                ex.assistant.trim()
            ));
        }
        out.push_str(&format!(
            "<|im_start|>user\n{}<|im_end|>\n\
             <|im_start|>assistant\n<think>\n\n</think>\n\n",
            added_text.trim()
        ));
        out
    }
}

fn home_dir() -> Option<PathBuf> {
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

pub fn resolve_model_path(configured_model_path: &str) -> Option<PathBuf> {
    let primary = expand_tilde(configured_model_path);
    if primary.is_file() {
        return Some(primary);
    }
    let fallback = expand_tilde(FALLBACK_MODEL_PATH);
    if fallback.is_file() {
        return Some(fallback);
    }
    None
}

pub fn resolve_tokenizer_path(configured_tokenizer_path: &str) -> Option<PathBuf> {
    let path = expand_tilde(configured_tokenizer_path);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn resolve_external_prompt_path(configured_prompt_path: Option<&str>, is_en: bool) -> Option<PathBuf> {
    if let Some(cfg_path) = configured_prompt_path {
        let trimmed = cfg_path.trim();
        if !trimmed.is_empty() {
            let path = expand_tilde(trimmed);
            if path.is_file() {
                return Some(path);
            }
            return None;
        }
    }

    if is_en {
        let en_path = expand_tilde(DEFAULT_USER_PROMPT_EN);
        if en_path.is_file() {
            return Some(en_path);
        }
        let ja_path = expand_tilde(DEFAULT_USER_PROMPT_JA);
        if ja_path.is_file() {
            return Some(ja_path);
        }
    } else {
        let ja_path = expand_tilde(DEFAULT_USER_PROMPT_JA);
        if ja_path.is_file() {
            return Some(ja_path);
        }
    }

    None
}

pub fn parse_prompt_or_fallback(toml_str: &str, is_en: bool) -> PromptTemplate {
    if let Ok(template) = toml::from_str::<PromptTemplate>(toml_str) {
        template
    } else {
        let default_str = if is_en {
            DEFAULT_EN_PROMPT_TOML
        } else {
            DEFAULT_JA_PROMPT_TOML
        };
        toml::from_str::<PromptTemplate>(default_str)
            .expect("embedded default prompt toml must be valid")
    }
}

pub fn load_prompt_template(configured_prompt_path: Option<&str>, is_en: bool) -> PromptTemplate {
    if let Some(path) = resolve_external_prompt_path(configured_prompt_path, is_en) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(template) = toml::from_str::<PromptTemplate>(&content) {
                return template;
            }
        }
    }
    parse_prompt_or_fallback("", is_en)
}

pub fn build_prompt(added_text: &str, is_en: bool, configured_prompt_path: Option<&str>) -> String {
    let template = load_prompt_template(configured_prompt_path, is_en);
    template.render(added_text)
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

fn build_logits_processor(cfg: &CoachConfig) -> LogitsProcessor {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);

    let sampling = if cfg.temperature <= 0.0 {
        Sampling::ArgMax
    } else if cfg.top_k > 0 && cfg.top_p > 0.0 && cfg.top_p < 1.0 {
        Sampling::TopKThenTopP {
            k: cfg.top_k,
            p: cfg.top_p,
            temperature: cfg.temperature,
        }
    } else if cfg.top_k > 0 {
        Sampling::TopK {
            k: cfg.top_k,
            temperature: cfg.temperature,
        }
    } else if cfg.top_p > 0.0 && cfg.top_p < 1.0 {
        Sampling::TopP {
            p: cfg.top_p,
            temperature: cfg.temperature,
        }
    } else {
        Sampling::All {
            temperature: cfg.temperature,
        }
    };

    LogitsProcessor::from_sampling(seed, sampling)
}

fn run_coach() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read single-line JSON payload from stdin
    let stdin = io::stdin();
    let mut first_line = String::new();
    let bytes_read = stdin.lock().read_line(&mut first_line)?;
    if bytes_read == 0 || first_line.trim().is_empty() {
        return Ok(());
    }

    let payload: OnReflectPayload = match serde_json::from_str(first_line.trim()) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let added_text = payload.added_text.trim();
    if added_text.is_empty() {
        return Ok(());
    }

    // 2. Load config from YARUBE_CONFIG_PATH or ~/.config/yarube/config.toml
    let raw_cfg_path = env::var("YARUBE_CONFIG_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());
    let config_path = expand_tilde(&raw_cfg_path);
    let coach_cfg = if config_path.is_file() {
        load_coach_config(&config_path)
    } else {
        CoachConfig::default()
    };

    if !coach_cfg.enabled || coach_cfg.max_tokens == 0 {
        return Ok(());
    }

    // 3. Resolve model and tokenizer paths; silently exit 0 if missing (BDD 08-1 / EARS-27)
    let Some(model_path) = resolve_model_path(&coach_cfg.model_path) else {
        return Ok(());
    };
    let Some(tokenizer_path) = resolve_tokenizer_path(&coach_cfg.tokenizer_path) else {
        return Ok(());
    };

    // 4. Load tokenizer and GGUF model on CPU
    let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(_) => return Ok(()),
    };

    let device = Device::Cpu;
    let mut model_file = match File::open(&model_path) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    let gguf_content = match gguf_file::Content::read(&mut model_file) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut model = match ModelWeights::from_gguf(gguf_content, &mut model_file, &device) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };

    // 5. Build ChatML + Non-Thinking prompt
    let lang_env = env::var("YARUBE_LANG").ok();
    let is_en = is_english_lang(lang_env.as_deref());
    let full_prompt = build_prompt(added_text, is_en, coach_cfg.prompt_path.as_deref());

    let encoding = match tokenizer.encode(full_prompt.as_str(), false) {
        Ok(enc) => enc,
        Err(_) => return Ok(()),
    };
    let prompt_tokens = encoding.get_ids();
    if prompt_tokens.is_empty() {
        return Ok(());
    }

    let im_end_id = tokenizer.token_to_id("<|im_end|>");
    let endoftext_id = tokenizer.token_to_id("<|endoftext|>");
    let is_eos = |token: u32| -> bool {
        im_end_id == Some(token) || endoftext_id == Some(token)
    };

    let mut logits_processor = build_logits_processor(&coach_cfg);

    // 6. Streaming output with 24bit True Color ANSI escape and multibyte protection (EARS-23)
    let color_hex = env::var("YARUBE_COLOR_COACH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_COACH_HEX.to_string());
    let (r, g, b) = parse_hex_rgb(&color_hex);

    print!("\x1b[38;2;{r};{g};{b}m\n> [Coach] ");
    let _ = io::stdout().flush();

    let mut generated_tokens: Vec<u32> = Vec::with_capacity(coach_cfg.max_tokens);
    let mut printed_len: usize = 0;

    // Prefill pass with full prompt
    let prompt_tensor = match Tensor::new(prompt_tokens, &device).and_then(|t| t.unsqueeze(0)) {
        Ok(t) => t,
        Err(_) => {
            println!("\x1b[0m");
            let _ = io::stdout().flush();
            return Ok(());
        }
    };

    let first_logits = match model.forward(&prompt_tensor, 0) {
        Ok(l) => match l.squeeze(0) {
            Ok(s) => s,
            Err(_) => {
                println!("\x1b[0m");
                let _ = io::stdout().flush();
                return Ok(());
            }
        },
        Err(_) => {
            println!("\x1b[0m");
            let _ = io::stdout().flush();
            return Ok(());
        }
    };

    let mut next_token = match logits_processor.sample(&first_logits) {
        Ok(t) => t,
        Err(_) => {
            println!("\x1b[0m");
            let _ = io::stdout().flush();
            return Ok(());
        }
    };

    if !is_eos(next_token) {
        generated_tokens.push(next_token);
        if let Ok(decoded) = tokenizer.decode(&generated_tokens, true) {
            if let Some(chunk) = extract_stream_chunk(&decoded, &mut printed_len) {
                print!("{chunk}");
                let _ = io::stdout().flush();
            }
        }

        for step in 1..coach_cfg.max_tokens {
            let offset = prompt_tokens.len() + step - 1;
            let input = match Tensor::new(&[next_token], &device).and_then(|t| t.unsqueeze(0)) {
                Ok(t) => t,
                Err(_) => break,
            };
            let logits = match model.forward(&input, offset).and_then(|l| l.squeeze(0)) {
                Ok(l) => l,
                Err(_) => break,
            };
            next_token = match logits_processor.sample(&logits) {
                Ok(t) => t,
                Err(_) => break,
            };
            if is_eos(next_token) {
                break;
            }
            generated_tokens.push(next_token);
            if let Ok(decoded) = tokenizer.decode(&generated_tokens, true) {
                if let Some(chunk) = extract_stream_chunk(&decoded, &mut printed_len) {
                    print!("{chunk}");
                    let _ = io::stdout().flush();
                }
            }
        }
    }

    println!("\x1b[0m");
    let _ = io::stdout().flush();

    // 7. Persist generated text to Markdown log file (EARS-25)
    if let Ok(full_decoded) = tokenizer.decode(&generated_tokens, true) {
        let log_path_str = if !payload.log_path.trim().is_empty() {
            payload.log_path.trim().to_string()
        } else if let Ok(env_log) = env::var("YARUBE_LOG_PATH") {
            env_log.trim().to_string()
        } else {
            String::new()
        };

        if !log_path_str.is_empty() {
            let log_path = expand_tilde(&log_path_str);
            let _ = append_coach_to_log(&log_path, &full_decoded);
        }
    }

    Ok(())
}

fn main() {
    let _ = std::panic::catch_unwind(|| {
        run_app();
    });
}

fn run_app() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "setup" => {
                std::process::exit(run_setup(&args[2..]));
            }
            "status" => {
                std::process::exit(run_status());
            }
            "--help" | "-h" | "help" => {
                println!("[yarube-coach]");
                println!("Usage:");
                println!("  yarube coach status");
                println!("  yarube coach setup [--no-download]");
                std::process::exit(0);
            }
            _ => {
                std::process::exit(run_status());
            }
        }
    }

    if io::stdin().is_terminal() {
        std::process::exit(run_status());
    }

    let _ = run_coach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), nanos));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_parse_hex_rgb() {
        assert_eq!(parse_hex_rgb("#7dcfff"), (0x7d, 0xcf, 0xff));
        assert_eq!(parse_hex_rgb("ae81ff"), (0xae, 0x81, 0xff));
        assert_eq!(parse_hex_rgb(" #8FBCBB "), (0x8f, 0xbc, 0xbb));
        assert_eq!(parse_hex_rgb("#6b705c"), (0x6b, 0x70, 0x5c));
        assert_eq!(parse_hex_rgb("invalid"), DEFAULT_COACH_RGB);
        assert_eq!(parse_hex_rgb(""), DEFAULT_COACH_RGB);
    }

    #[test]
    fn test_load_coach_config_defaults_and_custom() {
        let dir = unique_temp_dir("yarube_coach_cfg");
        let missing_path = dir.join("nonexistent.toml");
        let default_cfg = load_coach_config(&missing_path);
        assert_eq!(default_cfg, CoachConfig::default());
        assert!(default_cfg.enabled);
        assert_eq!(
            default_cfg.model_path,
            "~/.local/share/yarube/models/Qwen3-1.7B-Q8_0.gguf"
        );
        assert_eq!(
            default_cfg.tokenizer_path,
            "~/.local/share/yarube/models/tokenizer.json"
        );
        assert_eq!(default_cfg.prompt_path, None);
        assert_eq!(default_cfg.max_tokens, 100);
        assert!((default_cfg.temperature - 0.3).abs() < 1e-9);
        assert_eq!(default_cfg.top_k, 10);
        assert!((default_cfg.top_p - 0.8).abs() < 1e-9);

        let custom_path = dir.join("config.toml");
        std::fs::write(
            &custom_path,
            r#"
[general]
theme = "monokai"

[plugins.coach]
enabled = false
model_path = "~/custom/model.gguf"
tokenizer_path = "~/custom/tokenizer.json"
prompt_path = "~/custom/prompt.toml"
max_tokens = 64
temperature = 0.5
top_k = 20
top_p = 0.9
"#,
        )
        .unwrap();

        let custom_cfg = load_coach_config(&custom_path);
        assert!(!custom_cfg.enabled);
        assert_eq!(custom_cfg.model_path, "~/custom/model.gguf");
        assert_eq!(custom_cfg.tokenizer_path, "~/custom/tokenizer.json");
        assert_eq!(custom_cfg.prompt_path, Some("~/custom/prompt.toml".to_string()));
        assert_eq!(custom_cfg.max_tokens, 64);
        assert!((custom_cfg.temperature - 0.5).abs() < 1e-9);
        assert_eq!(custom_cfg.top_k, 20);
        assert!((custom_cfg.top_p - 0.9).abs() < 1e-9);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_default_prompts_parse_and_render() {
        // Japanese default prompt
        let template_ja: PromptTemplate = toml::from_str(DEFAULT_JA_PROMPT_TOML).expect("default_ja.toml should parse");
        assert!(!template_ja.examples.is_empty());
        let ja_rendered = template_ja.render("テストする気が起きない。");

        assert!(ja_rendered.contains("<|im_start|>system\n"));
        assert!(ja_rendered.contains("あなたは優しい内省コーチです。"));
        assert!(ja_rendered.contains("/no_think<|im_end|>\n"));
        assert!(ja_rendered.contains("<|im_start|>user\n設計で迷って予定が崩れた。自分はダメなエンジニアだ。<|im_end|>\n"));
        assert!(ja_rendered.contains("<|im_start|>assistant\n<think>\n\n</think>\n\n予定通りに進まず"));
        assert!(ja_rendered.contains("<|im_start|>user\nテストする気が起きない。<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"));

        // English default prompt
        let template_en: PromptTemplate = toml::from_str(DEFAULT_EN_PROMPT_TOML).expect("default_en.toml should parse");
        assert!(!template_en.examples.is_empty());
        let en_rendered = template_en.render("Too tired to work.");

        assert!(en_rendered.contains("<|im_start|>system\n"));
        assert!(en_rendered.contains("You are a gentle reflection coach."));
        assert!(en_rendered.contains("/no_think<|im_end|>\n"));
        assert!(en_rendered.contains("<|im_start|>user\nI got lost in the design and ruined my schedule. I'm a terrible engineer.<|im_end|>\n"));
        assert!(en_rendered.contains("<|im_start|>assistant\n<think>\n\n</think>\n\nIt must be so frustrating"));
        assert!(en_rendered.contains("<|im_start|>user\nToo tired to work.<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn test_multi_shot_custom_prompt_rendering() {
        let custom_toml = r#"
system = "カスタムコーチ指示"

[[examples]]
user = "1回目の悩み"
assistant = "1回目の返答"

[[examples]]
user = "2回目の悩み"
assistant = "2回目の返答"
"#;
        let template: PromptTemplate = toml::from_str(custom_toml).expect("custom toml should parse");
        assert_eq!(template.examples.len(), 2);
        let rendered = template.render("今回のふりかえり");

        // system automatically gains " /no_think"
        assert!(rendered.starts_with("<|im_start|>system\nカスタムコーチ指示 /no_think<|im_end|>\n"));
        // First shot
        assert!(rendered.contains("<|im_start|>user\n1回目の悩み<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n1回目の返答<|im_end|>\n"));
        // Second shot
        assert!(rendered.contains("<|im_start|>user\n2回目の悩み<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n2回目の返答<|im_end|>\n"));
        // Current user prompt
        assert!(rendered.ends_with("<|im_start|>user\n今回のふりかえり<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"));
    }

    #[test]
    fn test_invalid_toml_fallback() {
        let invalid_toml = "system = [unclosed syntax";
        // Should not panic, falls back to default_ja
        let template_ja = parse_prompt_or_fallback(invalid_toml, false);
        assert!(template_ja.system.contains("あなたは優しい内省コーチです。"));
        let rendered_ja = template_ja.render("入力");
        assert!(rendered_ja.contains("<think>\n\n</think>\n\n"));
        assert!(rendered_ja.contains("/no_think"));

        // Falls back to default_en
        let template_en = parse_prompt_or_fallback(invalid_toml, true);
        assert!(template_en.system.contains("You are a gentle reflection coach."));
        let rendered_en = template_en.render("input");
        assert!(rendered_en.contains("<think>\n\n</think>\n\n"));
        assert!(rendered_en.contains("/no_think"));
    }

    #[test]
    fn test_external_prompt_resolution_priority() {
        let dir = unique_temp_dir("yarube_coach_prompt_res");
        let custom_file = dir.join("my_coach.toml");
        std::fs::write(&custom_file, "system = 'my coach'").unwrap();

        // 1. Explicit config path wins
        let resolved = resolve_external_prompt_path(Some(custom_file.to_str().unwrap()), false);
        assert_eq!(resolved, Some(custom_file.clone()));

        // Non-existent explicit path returns None (falls back to built-in)
        let resolved_missing = resolve_external_prompt_path(Some("/path/to/nowhere.toml"), false);
        assert_eq!(resolved_missing, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_multibyte_stream_buffering_ears_23() {
        let mut printed_len = 0;

        // Simulate incomplete UTF-8 sequence ending with U+FFFD replacement char
        let partial = "お疲れ様\u{FFFD}";
        assert_eq!(extract_stream_chunk(partial, &mut printed_len), None);
        assert_eq!(printed_len, 0);

        // Once the multibyte char is completed, flush the full valid slice
        let completed = "お疲れ様です。\n質問：どこで止まりましたか？";
        let chunk = extract_stream_chunk(completed, &mut printed_len).unwrap();
        assert_eq!(chunk, "お疲れ様です。\n> 質問：どこで止まりましたか？");
        assert_eq!(printed_len, completed.len());

        // Subsequent call with no new bytes returns None
        assert_eq!(extract_stream_chunk(completed, &mut printed_len), None);
    }

    #[test]
    fn test_append_coach_to_log_ears_25() {
        let dir = unique_temp_dir("yarube_coach_log");
        let log_path = dir.join("2026-10-17.md");
        let initial = "### 10/17(火)\n\n#### △ やることリスト\n\n#### △ ふりかえり\nテストする気が起きない。\n";
        {
            let mut f = File::create(&log_path).unwrap();
            f.write_all(initial.as_bytes()).unwrap();
        }

        let generated = "  気持ちが向かずモヤモヤしているのですね。\n質問：最小の箇所はどこですか？ \n\n";
        append_coach_to_log(&log_path, generated).unwrap();

        let saved = std::fs::read_to_string(&log_path).unwrap();
        let expected = "### 10/17(火)\n\n#### △ やることリスト\n\n#### △ ふりかえり\nテストする気が起きない。\n> [Coach] 気持ちが向かずモヤモヤしているのですね。\n> 質問：最小の箇所はどこですか？\n";
        assert_eq!(saved, expected);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
