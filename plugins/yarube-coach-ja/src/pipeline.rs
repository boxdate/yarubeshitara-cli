use crate::{
    append_coach_to_log, build_en_coaching_prompt, expand_tilde, extract_stream_chunk,
    load_coach_ja_config, parse_hex_rgb, resolve_active_config_path, resolve_dialect_config,
    CoachJaConfig, OnReflectPayload, DEFAULT_COACH_HEX, FALLBACK_PLAMO_MODEL_PATH,
    FALLBACK_QWEN_MODEL_PATH,
};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_plamo3::{pfml3, Plamo3ModelWeights, Plamo3Tokenizer};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::quantized_qwen3::ModelWeights as Qwen3Model;
use candle_transformers::utils::apply_repeat_penalty;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokenizers::Tokenizer;

pub fn resolve_plamo_model_path(configured_path: &str) -> Option<PathBuf> {
    let primary = expand_tilde(configured_path);
    if primary.is_file() {
        return Some(primary);
    }
    let fallback = expand_tilde(FALLBACK_PLAMO_MODEL_PATH);
    if fallback.is_file() {
        return Some(fallback);
    }
    None
}

pub fn resolve_qwen_model_path(configured_path: &str) -> Option<PathBuf> {
    let primary = expand_tilde(configured_path);
    if primary.is_file() {
        return Some(primary);
    }
    let fallback = expand_tilde(FALLBACK_QWEN_MODEL_PATH);
    if fallback.is_file() {
        return Some(fallback);
    }
    None
}

pub fn resolve_qwen_tokenizer_path(configured_path: &str) -> Option<PathBuf> {
    let path = expand_tilde(configured_path);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn build_qwen_logits_processor(cfg: &CoachJaConfig) -> LogitsProcessor {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(2026);

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

pub struct LoadedPlamo3 {
    pub model: Plamo3ModelWeights,
    pub tokenizer: Plamo3Tokenizer,
}

impl LoadedPlamo3 {
    pub fn load(gguf_path: &Path, device: &Device) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(gguf_path)?;
        let content = gguf_file::Content::read(&mut file)?;
        let tokenizer = Plamo3Tokenizer::from_gguf(&content)?;
        let model = Plamo3ModelWeights::from_gguf(content, &mut file, device)?;
        Ok(Self { model, tokenizer })
    }

    pub fn clear_kv_cache(&mut self) {
        self.model.clear_kv_cache();
    }
}

pub fn is_plamo_stop_token(tokenizer: &Plamo3Tokenizer, token: u32) -> bool {
    if token == tokenizer.eos_token_id
        || token == tokenizer.eot_token_id
        || token == tokenizer.eom_token_id
    {
        return true;
    }
    if let Some(tok_str) = tokenizer.id_to_token(token) {
        if tok_str == "<|plamo:tag|>"
            || tok_str == "<|plamo:bos|>"
            || tok_str == "<|plamo:eos|>"
        {
            return true;
        }
    }
    false
}

pub fn trim_plamo_special_suffix(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(idx) = out.find("<|plamo:") {
        out.truncate(idx);
    }
    out.trim().to_string()
}

/// Step 1: Translates Japanese user input into English using PLaMo 3 Translate 0.8B (`Sampling::ArgMax`).
pub fn run_plamo3_step_with_instance(
    plamo: &mut LoadedPlamo3,
    prompt: &str,
    device: &Device,
    max_new_tokens: usize,
    mut on_chunk: Option<&mut dyn FnMut(&str)>,
) -> Result<String, Box<dyn std::error::Error>> {
    let prompt_tokens = plamo.tokenizer.encode(prompt, true)?;
    if prompt_tokens.is_empty() {
        return Ok(String::new());
    }
    let prompt_len = prompt_tokens.len();

    // Greedy decoding (Sampling::ArgMax / temperature = 0.0) as required by PLaMo 3 Translate spec §4.3
    let mut logits_processor = LogitsProcessor::from_sampling(2026, Sampling::ArgMax);
    let mut generated_tokens = Vec::with_capacity(max_new_tokens);
    let mut next_token = 0u32;
    let mut printed_len = 0usize;

    for step in 0..max_new_tokens {
        let (input_tensor, pos) = if step == 0 {
            let tensor = Tensor::new(prompt_tokens.as_slice(), device)?.unsqueeze(0)?;
            (tensor, 0)
        } else {
            let tensor = Tensor::new(&[next_token], device)?.unsqueeze(0)?;
            (tensor, prompt_len + step - 1)
        };

        let logits = plamo.model.forward(&input_tensor, pos)?.squeeze(0)?;
        next_token = logits_processor.sample(&logits)?;

        if is_plamo_stop_token(&plamo.tokenizer, next_token) {
            break;
        }

        generated_tokens.push(next_token);

        let curr_raw = plamo.tokenizer.decode(&generated_tokens, false)?;
        if curr_raw.contains("<|plamo:tag|>") || curr_raw.contains("<|plamo:bos|>") {
            break;
        }

        if let Some(ref mut cb) = on_chunk {
            if let Ok(decoded_clean) = plamo.tokenizer.decode(&generated_tokens, true) {
                let clean_view = match decoded_clean.find("<|plamo:") {
                    Some(idx) => &decoded_clean[..idx],
                    None => decoded_clean.as_str(),
                };
                if let Some(chunk) = extract_stream_chunk(clean_view, &mut printed_len) {
                    cb(&chunk);
                }
            }
        }
    }

    let decoded = plamo.tokenizer.decode(&generated_tokens, true)?;
    Ok(trim_plamo_special_suffix(&decoded))
}

/// Step 2: Runs Qwen3-1.7B in English to generate 1 empathetic sentence + 1 concrete question,
/// then explicitly `drop(qwen_model)` to free memory.
pub fn run_qwen3_en_coaching(
    gguf_path: &Path,
    tokenizer_path: &Path,
    en_user_memo: &str,
    cfg: &CoachJaConfig,
    device: &Device,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(gguf_path)?;
    let content = gguf_file::Content::read(&mut file)?;
    let mut qwen_model = Qwen3Model::from_gguf(content, &mut file, device)?;
    let tokenizer =
        Tokenizer::from_file(tokenizer_path).map_err(|e| format!("Tokenizer load error: {e}"))?;

    let full_prompt = build_en_coaching_prompt(en_user_memo, cfg.prompt_path.as_deref());
    let prompt_tokens = tokenizer
        .encode(full_prompt, true)
        .map_err(|e| format!("Tokenize error: {e}"))?
        .get_ids()
        .to_vec();
    if prompt_tokens.is_empty() {
        return Ok(String::new());
    }
    let prompt_len = prompt_tokens.len();

    let mut logits_processor = build_qwen_logits_processor(cfg);
    let eos_token_id = tokenizer.token_to_id("<|im_end|>").unwrap_or(151645);
    let eot_token_id = tokenizer.token_to_id("<|endoftext|>").unwrap_or(151643);

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
        if (cfg.repeat_penalty - 1.0).abs() > 1e-6 && !ctx.is_empty() {
            apply_repeat_penalty(&logits, cfg.repeat_penalty, ctx).unwrap_or(logits)
        } else {
            logits
        }
    };

    let mut generated_tokens = Vec::with_capacity(cfg.step2_max_tokens);
    let mut next_token = 0u32;

    for step in 0..cfg.step2_max_tokens {
        let (input_tensor, pos) = if step == 0 {
            let tensor = Tensor::new(prompt_tokens.as_slice(), device)?.unsqueeze(0)?;
            (tensor, 0)
        } else {
            let tensor = Tensor::new(&[next_token], device)?.unsqueeze(0)?;
            (tensor, prompt_len + step - 1)
        };

        let logits = qwen_model.forward(&input_tensor, pos)?.squeeze(0)?;
        let ctx = get_penalty_context(&generated_tokens, &prompt_tokens, cfg.repeat_last_n);
        let logits = apply_penalty(logits, &ctx);
        next_token = logits_processor.sample(&logits)?;

        if next_token == eos_token_id || next_token == eot_token_id {
            break;
        }
        generated_tokens.push(next_token);
    }

    let output_text = tokenizer
        .decode(&generated_tokens, true)
        .map_err(|e| format!("Decode error: {e}"))?
        .trim()
        .to_string();

    // Explicitly drop Qwen3 model weights right after Step 2
    drop(qwen_model);

    Ok(output_text)
}

/// Runs the full 3-step `yarube-coach-ja` pipeline triggered by `on_reflect` hook.
pub fn run_coach_ja_pipeline() -> Result<(), Box<dyn std::error::Error>> {
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
    let config_path = resolve_active_config_path();
    let cfg = if config_path.is_file() {
        load_coach_ja_config(&config_path)
    } else {
        CoachJaConfig::default()
    };

    if !cfg.enabled
        || cfg.step1_max_tokens == 0
        || cfg.step2_max_tokens == 0
        || cfg.step3_max_tokens == 0
    {
        return Ok(());
    }

    // 3. Resolve model & tokenizer files; silently exit 0 if missing (EARS-27)
    let Some(plamo_path) = resolve_plamo_model_path(&cfg.plamo_model_path) else {
        return Ok(());
    };
    let Some(qwen_path) = resolve_qwen_model_path(&cfg.qwen_model_path) else {
        return Ok(());
    };
    let Some(qwen_tok_path) = resolve_qwen_tokenizer_path(&cfg.qwen_tokenizer_path) else {
        return Ok(());
    };

    let resolved_dialect = resolve_dialect_config(&cfg);
    let device = Device::Cpu;

    // ------------------------------------------------------------------
    // Step 1: JA -> EN (PLaMo 3 Translate 0.8B)
    // ------------------------------------------------------------------
    let prompt_ja_en = pfml3::prompt_ja_to_en(added_text);
    let mut cached_plamo: Option<LoadedPlamo3> = None;

    let en_memo = {
        let mut plamo = match LoadedPlamo3::load(&plamo_path, &device) {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        let translated = match run_plamo3_step_with_instance(
            &mut plamo,
            &prompt_ja_en,
            &device,
            cfg.step1_max_tokens,
            None,
        ) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };

        if cfg.keep_plamo_in_memory {
            plamo.clear_kv_cache();
            cached_plamo = Some(plamo);
        } else {
            // Explicitly drop PLaMo 3 weights before loading Qwen3 to keep peak RAM low
            drop(plamo);
        }
        translated
    };

    if en_memo.trim().is_empty() {
        return Ok(());
    }

    // ------------------------------------------------------------------
    // Step 2: EN Reflection Coaching (Qwen3-1.7B)
    // ------------------------------------------------------------------
    let en_coaching = match run_qwen3_en_coaching(
        &qwen_path,
        &qwen_tok_path,
        &en_memo,
        &cfg,
        &device,
    ) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    if en_coaching.trim().is_empty() {
        return Ok(());
    }

    // ------------------------------------------------------------------
    // Step 3: EN -> JA / Dialect (PLaMo 3 Translate 0.8B) with streaming output
    // ------------------------------------------------------------------
    let prompt_en_ja = pfml3::prompt_en_to_ja_with_config(&en_coaching, &resolved_dialect.config);

    let color_hex = env::var("YARUBE_COLOR_COACH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_COACH_HEX.to_string());
    let (r, g, b) = parse_hex_rgb(&color_hex);

    print!("\x1b[38;2;{r};{g};{b}m\n> [Coach] ");
    let _ = io::stdout().flush();

    let mut plamo_step3 = match cached_plamo {
        Some(p) => p,
        None => match LoadedPlamo3::load(&plamo_path, &device) {
            Ok(p) => p,
            Err(_) => {
                println!("\x1b[0m");
                let _ = io::stdout().flush();
                return Ok(());
            }
        },
    };

    let mut print_cb = |chunk: &str| {
        print!("{chunk}");
        let _ = io::stdout().flush();
    };

    let final_ja_result = run_plamo3_step_with_instance(
        &mut plamo_step3,
        &prompt_en_ja,
        &device,
        cfg.step3_max_tokens,
        Some(&mut print_cb),
    );

    // Explicitly drop PLaMo 3 model weights after Step 3
    drop(plamo_step3);

    println!("\x1b[0m");
    let _ = io::stdout().flush();

    // 4. Append generated coaching text to daily Markdown log file (EARS-25)
    if let Ok(final_ja) = final_ja_result {
        if !final_ja.trim().is_empty() {
            let log_path_str = if !payload.log_path.trim().is_empty() {
                payload.log_path.trim().to_string()
            } else if let Ok(env_log) = env::var("YARUBE_LOG_PATH") {
                env_log.trim().to_string()
            } else {
                String::new()
            };

            if !log_path_str.is_empty() {
                let log_path = expand_tilde(&log_path_str);
                let _ = append_coach_to_log(&log_path, &final_ja);
            }
        }
    }

    Ok(())
}
