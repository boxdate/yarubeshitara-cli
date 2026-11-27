use crate::{
    ensure_default_dialect_files, expand_tilde, list_available_dialects, load_coach_ja_config,
    normalize_preset_id, resolve_active_config_path, resolve_dialect_by_spec,
    resolve_dialect_config, resolve_external_prompt_path, resolve_plamo_model_path,
    resolve_qwen_model_path, resolve_qwen_tokenizer_path, update_config_dialect, CoachJaConfig,
    DEFAULT_EN_PROMPT_TOML, DEFAULT_USER_PROMPT_EN,
};
use std::io::{self, BufRead, Write};
use std::path::Path;

pub const HF_PLAMO_MODEL_URL: &str =
    "https://huggingface.co/pfnet/plamo-3-translate-0.8b-GGUF/resolve/main/plamo-3-translate-0.8b-Q4_K_M.gguf";
pub const HF_QWEN_MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf";
pub const HF_QWEN_TOKENIZER_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-1.7B/resolve/main/tokenizer.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Status,
    Setup(Vec<String>),
    Dialect(Option<String>),
    Help,
    Inference,
}

pub fn parse_cli_args(args: &[String]) -> CliCommand {
    if args.len() > 1 {
        match args[1].as_str() {
            "setup" => CliCommand::Setup(args[2..].to_vec()),
            "status" => CliCommand::Status,
            "dialect" | "dialects" => {
                if args.len() > 2 {
                    let target = args[2..].join(" ").trim().to_string();
                    if target.is_empty() {
                        CliCommand::Dialect(None)
                    } else {
                        CliCommand::Dialect(Some(target))
                    }
                } else {
                    CliCommand::Dialect(None)
                }
            }
            "--help" | "-h" | "help" => CliCommand::Help,
            _ => CliCommand::Status,
        }
    } else {
        CliCommand::Inference
    }
}

fn invoke_curl(output_path: &Path, url: &str) -> bool {
    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-L");
    cmd.arg("--fail");
    cmd.arg("-o");
    cmd.arg(output_path);
    cmd.arg(url);
    match cmd.status() {
        Ok(s) => s.success(),
        Err(_) => false,
    }
}

pub fn run_status_to(out: &mut dyn Write) -> io::Result<()> {
    let config_path = resolve_active_config_path();
    let cfg = if config_path.is_file() {
        load_coach_ja_config(&config_path)
    } else {
        CoachJaConfig::default()
    };

    writeln!(out, "[yarube-coach-ja status]")?;

    let plamo_path = resolve_plamo_model_path(&cfg.plamo_model_path)
        .unwrap_or_else(|| expand_tilde(&cfg.plamo_model_path));
    let plamo_status = if plamo_path.is_file() {
        "[OK]"
    } else {
        "[MISSING]"
    };
    writeln!(
        out,
        "- PLaMo 3 model  : {} {}",
        plamo_path.display(),
        plamo_status
    )?;

    let qwen_path = resolve_qwen_model_path(&cfg.qwen_model_path)
        .unwrap_or_else(|| expand_tilde(&cfg.qwen_model_path));
    let qwen_status = if qwen_path.is_file() {
        "[OK]"
    } else {
        "[MISSING]"
    };
    writeln!(
        out,
        "- Qwen3 model    : {} {}",
        qwen_path.display(),
        qwen_status
    )?;

    let tok_path = resolve_qwen_tokenizer_path(&cfg.qwen_tokenizer_path)
        .unwrap_or_else(|| expand_tilde(&cfg.qwen_tokenizer_path));
    let tok_status = if tok_path.is_file() {
        "[OK]"
    } else {
        "[MISSING]"
    };
    writeln!(
        out,
        "- Qwen3 tokenizer: {} {}",
        tok_path.display(),
        tok_status
    )?;

    let resolved_dialect = resolve_dialect_config(&cfg);
    let dialect_source = match &resolved_dialect.source_path {
        Some(p) => format!("{} [JSON]", p.display()),
        None => format!("{} [BUILT-IN]", resolved_dialect.id),
    };
    writeln!(
        out,
        "- Active dialect : {} ({} / glossary: {}, few_shots: {})",
        resolved_dialect.config.name,
        dialect_source,
        resolved_dialect.config.glossary.len(),
        resolved_dialect.config.few_shots.len()
    )?;

    let prompt_resolved = resolve_external_prompt_path(cfg.prompt_path.as_deref());
    let (prompt_path, prompt_status) = if let Some(p) = prompt_resolved {
        (p, "[CUSTOM]")
    } else {
        (
            expand_tilde(DEFAULT_USER_PROMPT_EN),
            "[DEFAULT (built-in)]",
        )
    };
    writeln!(
        out,
        "- Prompt config  : {} {}",
        prompt_path.display(),
        prompt_status
    )?;

    let mem_policy = if cfg.keep_plamo_in_memory {
        "keep_plamo_in_memory (reuse PLaMo 3 across Step 1 & Step 3)"
    } else {
        "sequential_drop (explicitly drop after each step)"
    };
    writeln!(out, "- Memory policy  : {}", mem_policy)?;

    Ok(())
}

pub fn run_status() -> i32 {
    let mut stdout = io::stdout();
    let _ = run_status_to(&mut stdout);
    0
}

pub fn run_setup_to(out: &mut dyn Write, args: &[String]) -> io::Result<()> {
    let config_path = resolve_active_config_path();
    let cfg = if config_path.is_file() {
        load_coach_ja_config(&config_path)
    } else {
        CoachJaConfig::default()
    };

    // 1. Ensure models, config, and dialects directories exist
    let models_dir = expand_tilde("~/.local/share/yarube/models");
    let config_dir = expand_tilde("~/.config/yarube");
    let dialects_dir = expand_tilde(&cfg.dialects_dir);
    let _ = std::fs::create_dir_all(&models_dir);
    let _ = std::fs::create_dir_all(&config_dir);
    let _ = std::fs::create_dir_all(&dialects_dir);

    // 2. Write default dialect JSON files (standard_polite.json, hokkaido_donan.json, kansai.json)
    let created_dialects = ensure_default_dialect_files(&dialects_dir);
    for p in created_dialects {
        writeln!(out, "[OK] Created dialect config: {}", p.display())?;
    }

    // 3. Create default English coaching prompt config ~/.config/yarube/coach_ja_prompt.toml if not present
    let prompt_path = expand_tilde(DEFAULT_USER_PROMPT_EN);
    if !prompt_path.exists() && std::fs::write(&prompt_path, DEFAULT_EN_PROMPT_TOML).is_ok() {
        writeln!(
            out,
            "[OK] Created default prompt config: {}",
            prompt_path.display()
        )?;
    }

    // 4. Check PLaMo 3 and Qwen3 model/tokenizer files
    let plamo_path = expand_tilde(&cfg.plamo_model_path);
    let qwen_path = expand_tilde(&cfg.qwen_model_path);
    let tok_path = expand_tilde(&cfg.qwen_tokenizer_path);

    let has_plamo =
        plamo_path.is_file() || resolve_plamo_model_path(&cfg.plamo_model_path).is_some();
    let has_qwen = qwen_path.is_file() || resolve_qwen_model_path(&cfg.qwen_model_path).is_some();
    let has_tok =
        tok_path.is_file() || resolve_qwen_tokenizer_path(&cfg.qwen_tokenizer_path).is_some();

    if has_plamo && has_qwen && has_tok {
        writeln!(
            out,
            "[OK] All models (PLaMo 3 Translate + Qwen3) and tokenizer are already installed."
        )?;
        return Ok(());
    }

    let no_download = args.iter().any(|a| a == "--no-download");
    if no_download {
        writeln!(
            out,
            "[INFO] To download the recommended models and tokenizer manually, run:"
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            plamo_path.display(),
            HF_PLAMO_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            qwen_path.display(),
            HF_QWEN_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            tok_path.display(),
            HF_QWEN_TOKENIZER_URL
        )?;
        return Ok(());
    }

    let mut curl_failed = false;
    if !has_plamo {
        writeln!(
            out,
            "[INFO] Downloading PLaMo 3 Translate 0.8B to {}...",
            plamo_path.display()
        )?;
        if invoke_curl(&plamo_path, HF_PLAMO_MODEL_URL) {
            writeln!(out, "[OK] Downloaded PLaMo 3 model successfully.")?;
        } else {
            writeln!(out, "[INFO] curl failed to download PLaMo 3 model.")?;
            curl_failed = true;
        }
    }

    if !has_qwen && !curl_failed {
        writeln!(
            out,
            "[INFO] Downloading Qwen3-1.7B to {}...",
            qwen_path.display()
        )?;
        if invoke_curl(&qwen_path, HF_QWEN_MODEL_URL) {
            writeln!(out, "[OK] Downloaded Qwen3 model successfully.")?;
        } else {
            writeln!(out, "[INFO] curl failed to download Qwen3 model.")?;
            curl_failed = true;
        }
    }

    if !has_tok && !curl_failed {
        writeln!(
            out,
            "[INFO] Downloading Qwen3 tokenizer to {}...",
            tok_path.display()
        )?;
        if invoke_curl(&tok_path, HF_QWEN_TOKENIZER_URL) {
            writeln!(out, "[OK] Downloaded Qwen3 tokenizer successfully.")?;
        } else {
            writeln!(out, "[INFO] curl failed to download Qwen3 tokenizer.")?;
            curl_failed = true;
        }
    }

    if curl_failed {
        writeln!(out, "[INFO] To download manually, run:")?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            plamo_path.display(),
            HF_PLAMO_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            qwen_path.display(),
            HF_QWEN_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            tok_path.display(),
            HF_QWEN_TOKENIZER_URL
        )?;
    } else {
        writeln!(out, "[OK] Model and tokenizer setup complete.")?;
    }

    Ok(())
}

pub fn run_setup(args: &[String]) -> i32 {
    let mut stdout = io::stdout();
    let _ = run_setup_to(&mut stdout, args);
    0
}

pub fn run_dialect_with_io<R: BufRead, W: Write>(
    direct_target: Option<&str>,
    reader: &mut R,
    out: &mut W,
) -> io::Result<()> {
    let config_path = resolve_active_config_path();
    let cfg = if config_path.is_file() {
        load_coach_ja_config(&config_path)
    } else {
        CoachJaConfig::default()
    };

    let dialects_dir = expand_tilde(&cfg.dialects_dir);
    let _ = ensure_default_dialect_files(&dialects_dir);
    let available = list_available_dialects(&cfg.dialects_dir);
    let active = resolve_dialect_config(&cfg);

    let chosen_spec = if let Some(target) = direct_target {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        if let Ok(num) = trimmed.parse::<usize>() {
            if num >= 1 && num <= available.len() {
                available[num - 1].id.clone()
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid dialect number: {}", num),
                ));
            }
        } else if let Some(preset) = normalize_preset_id(trimmed) {
            preset.id.to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        writeln!(out, "[yarube-coach-ja dialect]")?;
        writeln!(
            out,
            "現在の口調・方言: {} (`{}`)",
            active.config.name, active.id
        )?;
        writeln!(out)?;
        for (idx, item) in available.iter().enumerate() {
            let marker = if item.id == active.id { "*" } else { " " };
            writeln!(
                out,
                " {} {}) {:<16} - {} (glossary: {}, few_shots: {})",
                marker,
                idx + 1,
                item.id,
                item.config.name,
                item.config.glossary.len(),
                item.config.few_shots.len()
            )?;
        }
        writeln!(out)?;
        write!(
            out,
            "番号 (1-{}) または方言名/JSONパスを入力してください: ",
            available.len()
        )?;
        out.flush()?;

        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            writeln!(out)?;
            return Ok(());
        }
        let choice = line.trim();
        if choice.is_empty() {
            return Ok(());
        }
        if let Ok(num) = choice.parse::<usize>() {
            if num >= 1 && num <= available.len() {
                available[num - 1].id.clone()
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Invalid dialect number: {}", num),
                ));
            }
        } else if let Some(preset) = normalize_preset_id(choice) {
            preset.id.to_string()
        } else {
            choice.to_string()
        }
    };

    let resolved = resolve_dialect_by_spec(&chosen_spec, &cfg.dialects_dir);
    update_config_dialect(&config_path, &chosen_spec)?;

    writeln!(
        out,
        "[OK] コーチの方言・口調を `{}` ({}) に設定しました。",
        chosen_spec, resolved.config.name
    )?;

    Ok(())
}

pub fn run_dialect(direct_target: Option<&str>) -> i32 {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();
    match run_dialect_with_io(direct_target, &mut reader, &mut stdout) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[ERROR] {e}");
            1
        }
    }
}
