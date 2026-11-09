use crate::{
    expand_tilde, is_english_lang, load_coach_config, resolve_external_prompt_path,
    resolve_model_path, resolve_tokenizer_path, CoachConfig, DEFAULT_CONFIG_PATH,
    DEFAULT_EN_PROMPT_TOML, DEFAULT_JA_PROMPT_TOML, DEFAULT_USER_PROMPT_EN,
    DEFAULT_USER_PROMPT_JA,
};
use std::env;
use std::io::{self, Write};
use std::path::Path;

pub const HF_MODEL_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q8_0.gguf";
pub const HF_TOKENIZER_URL: &str =
    "https://huggingface.co/Qwen/Qwen3-1.7B/resolve/main/tokenizer.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Status,
    Setup(Vec<String>),
    Help,
    Inference,
}

pub fn parse_cli_args(args: &[String]) -> CliCommand {
    if args.len() > 1 {
        match args[1].as_str() {
            "setup" => CliCommand::Setup(args[2..].to_vec()),
            "status" => CliCommand::Status,
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

    writeln!(out, "[yarube-coach status]")?;

    let model_path = resolve_model_path(&coach_cfg.model_path)
        .unwrap_or_else(|| expand_tilde(&coach_cfg.model_path));
    let model_status = if model_path.is_file() {
        "[OK]"
    } else {
        "[MISSING]"
    };
    writeln!(
        out,
        "- Model file     : {} {}",
        model_path.display(),
        model_status
    )?;

    let tokenizer_path = resolve_tokenizer_path(&coach_cfg.tokenizer_path)
        .unwrap_or_else(|| expand_tilde(&coach_cfg.tokenizer_path));
    let tokenizer_status = if tokenizer_path.is_file() {
        "[OK]"
    } else {
        "[MISSING]"
    };
    writeln!(
        out,
        "- Tokenizer file : {} {}",
        tokenizer_path.display(),
        tokenizer_status
    )?;

    let lang_env = env::var("YARUBE_LANG").ok();
    let is_en = is_english_lang(lang_env.as_deref());
    let prompt_resolved = resolve_external_prompt_path(coach_cfg.prompt_path.as_deref(), is_en);
    let (prompt_path, prompt_status) = if let Some(p) = prompt_resolved {
        (p, "[CUSTOM]")
    } else {
        let default_rel = if is_en {
            DEFAULT_USER_PROMPT_EN
        } else {
            DEFAULT_USER_PROMPT_JA
        };
        (expand_tilde(default_rel), "[DEFAULT (built-in)]")
    };
    writeln!(
        out,
        "- Prompt config  : {} {}",
        prompt_path.display(),
        prompt_status
    )?;

    Ok(())
}

pub fn run_status() -> i32 {
    let mut stdout = io::stdout();
    let _ = run_status_to(&mut stdout);
    0
}

pub fn run_setup_to(out: &mut dyn Write, args: &[String]) -> io::Result<()> {
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

    // 1. Create ~/.local/share/yarube/models and ~/.config/yarube if not present
    let models_dir = expand_tilde("~/.local/share/yarube/models");
    let config_dir = expand_tilde("~/.config/yarube");
    if !models_dir.exists() {
        let _ = std::fs::create_dir_all(&models_dir);
    }
    if !config_dir.exists() {
        let _ = std::fs::create_dir_all(&config_dir);
    }

    // 2. Create default prompt config ~/.config/yarube/coach_prompt.toml if not present
    let lang_env = env::var("YARUBE_LANG").ok();
    let is_en = is_english_lang(lang_env.as_deref());
    let prompt_path = expand_tilde(DEFAULT_USER_PROMPT_JA);
    if !prompt_path.exists() {
        let default_content = if is_en {
            DEFAULT_EN_PROMPT_TOML
        } else {
            DEFAULT_JA_PROMPT_TOML
        };
        if std::fs::write(&prompt_path, default_content).is_ok() {
            writeln!(
                out,
                "[OK] Created default prompt config: {}",
                prompt_path.display()
            )?;
        }
    }

    // 3. Check model and tokenizer files
    let model_path = expand_tilde(&coach_cfg.model_path);
    let tokenizer_path = expand_tilde(&coach_cfg.tokenizer_path);

    let has_model = model_path.is_file() || resolve_model_path(&coach_cfg.model_path).is_some();
    let has_tokenizer = tokenizer_path.is_file()
        || resolve_tokenizer_path(&coach_cfg.tokenizer_path).is_some();

    if has_model && has_tokenizer {
        writeln!(out, "[OK] Model and tokenizer are already installed.")?;
        return Ok(());
    }

    let no_download = args.iter().any(|a| a == "--no-download");
    if no_download {
        writeln!(
            out,
            "[INFO] To download the recommended model and tokenizer manually, run:"
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            model_path.display(),
            HF_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            tokenizer_path.display(),
            HF_TOKENIZER_URL
        )?;
        return Ok(());
    }

    let mut curl_failed = false;
    if !has_model {
        writeln!(
            out,
            "[INFO] Downloading model to {}...",
            model_path.display()
        )?;
        if invoke_curl(&model_path, HF_MODEL_URL) {
            writeln!(out, "[OK] Downloaded model successfully.")?;
        } else {
            writeln!(out, "[INFO] curl failed to download model.")?;
            curl_failed = true;
        }
    }

    if !has_tokenizer && !curl_failed {
        writeln!(
            out,
            "[INFO] Downloading tokenizer to {}...",
            tokenizer_path.display()
        )?;
        if invoke_curl(&tokenizer_path, HF_TOKENIZER_URL) {
            writeln!(out, "[OK] Downloaded tokenizer successfully.")?;
        } else {
            writeln!(out, "[INFO] curl failed to download tokenizer.")?;
            curl_failed = true;
        }
    }

    if curl_failed {
        writeln!(out, "[INFO] To download manually, run:")?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            model_path.display(),
            HF_MODEL_URL
        )?;
        writeln!(
            out,
            "  curl -L -o \"{}\" \"{}\"",
            tokenizer_path.display(),
            HF_TOKENIZER_URL
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
