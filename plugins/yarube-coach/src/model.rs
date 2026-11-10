use crate::{
    append_coach_to_log, build_prompt, expand_tilde, extract_stream_chunk, is_english_lang,
    load_coach_config, parse_hex_rgb, CoachConfig, OnReflectPayload, DEFAULT_COACH_HEX,
    DEFAULT_CONFIG_PATH, FALLBACK_MODEL_PATH,
};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights;
use candle_transformers::utils::apply_repeat_penalty;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;

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

pub fn build_logits_processor(cfg: &CoachConfig) -> LogitsProcessor {
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

pub fn run_coach() -> Result<(), Box<dyn std::error::Error>> {
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

    let get_penalty_context = |gen: &[u32], prompt: &[u32], n: usize| -> Vec<u32> {
        if n == 0 {
            return Vec::new();
        }
        if gen.len() >= n {
            gen[gen.len() - n..].to_vec()
        } else {
            let needed = n - gen.len();
            let p_start = prompt.len().saturating_sub(needed);
            let mut ctx = Vec::with_capacity(n);
            ctx.extend_from_slice(&prompt[p_start..]);
            ctx.extend_from_slice(gen);
            ctx
        }
    };

    let apply_penalty = |logits: Tensor, ctx: &[u32]| -> Tensor {
        if (coach_cfg.repeat_penalty - 1.0).abs() > 1e-6 && !ctx.is_empty() {
            apply_repeat_penalty(&logits, coach_cfg.repeat_penalty, ctx).unwrap_or(logits)
        } else {
            logits
        }
    };

    let first_ctx = get_penalty_context(&[], prompt_tokens, coach_cfg.repeat_last_n);
    let first_logits = apply_penalty(first_logits, &first_ctx);
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
            let ctx = get_penalty_context(&generated_tokens, prompt_tokens, coach_cfg.repeat_last_n);
            let logits = apply_penalty(logits, &ctx);
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
