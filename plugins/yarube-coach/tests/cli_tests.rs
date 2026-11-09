use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use yarube_coach::cli::{
    parse_cli_args, run_setup_to, run_status_to, CliCommand, HF_MODEL_URL, HF_TOKENIZER_URL,
};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

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
fn test_parse_cli_args_normal_and_abnormal() {
    // No subcommand (Inference mode)
    let args_none = vec!["yarube-coach".to_string()];
    assert_eq!(parse_cli_args(&args_none), CliCommand::Inference);

    // Empty slice edge case
    let args_empty: Vec<String> = vec![];
    assert_eq!(parse_cli_args(&args_empty), CliCommand::Inference);

    // Status subcommand
    let args_status = vec!["yarube-coach".to_string(), "status".to_string()];
    assert_eq!(parse_cli_args(&args_status), CliCommand::Status);

    // Setup subcommand without flags
    let args_setup = vec!["yarube-coach".to_string(), "setup".to_string()];
    assert_eq!(parse_cli_args(&args_setup), CliCommand::Setup(vec![]));

    // Setup subcommand with --no-download
    let args_setup_no_dl = vec![
        "yarube-coach".to_string(),
        "setup".to_string(),
        "--no-download".to_string(),
    ];
    assert_eq!(
        parse_cli_args(&args_setup_no_dl),
        CliCommand::Setup(vec!["--no-download".to_string()])
    );

    // Help flags
    for flag in ["--help", "-h", "help"] {
        let args_help = vec!["yarube-coach".to_string(), flag.to_string()];
        assert_eq!(parse_cli_args(&args_help), CliCommand::Help);
    }

    // Abnormal / unknown subcommand falls back to Status
    let args_unknown = vec!["yarube-coach".to_string(), "unknown-subcmd".to_string()];
    assert_eq!(parse_cli_args(&args_unknown), CliCommand::Status);

    let args_invalid_flag = vec!["yarube-coach".to_string(), "--invalid".to_string()];
    assert_eq!(parse_cli_args(&args_invalid_flag), CliCommand::Status);
}

#[test]
fn test_status_output_missing_and_ok_states() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = unique_temp_dir("yarube_coach_status_test");
    let fake_home = dir.join("home");
    std::fs::create_dir_all(&fake_home).unwrap();

    let orig_home = env::var("HOME").ok();
    env::set_var("HOME", &fake_home);

    let mut buf_missing = Vec::new();
    run_status_to(&mut buf_missing).unwrap();
    let out_missing = String::from_utf8(buf_missing).unwrap();

    assert!(out_missing.contains("[yarube-coach status]"));
    assert!(out_missing.contains("- Model file     :"));
    assert!(out_missing.contains("[MISSING]"));
    assert!(out_missing.contains("- Tokenizer file :"));
    assert!(out_missing.contains("- Prompt config  :"));
    assert!(out_missing.contains("[DEFAULT (built-in)]"));

    assert!(
        !out_missing.contains("❌")
            && !out_missing.contains("✅")
            && !out_missing.contains("🤖")
    );

    let models_dir = fake_home.join(".local/share/yarube/models");
    let config_dir = fake_home.join(".config/yarube");
    std::fs::create_dir_all(&models_dir).unwrap();
    std::fs::create_dir_all(&config_dir).unwrap();

    std::fs::write(models_dir.join("Qwen3-1.7B-Q8_0.gguf"), b"dummy model").unwrap();
    std::fs::write(models_dir.join("tokenizer.json"), b"dummy tokenizer").unwrap();
    std::fs::write(config_dir.join("coach_prompt.toml"), b"system = 'dummy'").unwrap();

    let mut buf_ok = Vec::new();
    run_status_to(&mut buf_ok).unwrap();
    let out_ok = String::from_utf8(buf_ok).unwrap();

    assert!(out_ok.contains("- Model file     :"));
    assert!(out_ok.contains("[OK]"));
    assert!(out_ok.contains("- Tokenizer file :"));
    assert!(out_ok.contains("- Prompt config  :"));
    assert!(out_ok.contains("[CUSTOM]"));

    if let Some(h) = orig_home {
        env::set_var("HOME", h);
    } else {
        env::remove_var("HOME");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_setup_creates_default_prompt_and_handles_no_download() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = unique_temp_dir("yarube_coach_setup_test");
    let fake_home = dir.join("home");
    std::fs::create_dir_all(&fake_home).unwrap();

    let orig_home = env::var("HOME").ok();
    env::set_var("HOME", &fake_home);

    let mut buf_setup = Vec::new();
    run_setup_to(&mut buf_setup, &["--no-download".to_string()]).unwrap();
    let out_setup = String::from_utf8(buf_setup).unwrap();

    let prompt_path = fake_home.join(".config/yarube/coach_prompt.toml");
    assert!(prompt_path.is_file(), "coach_prompt.toml must be created");
    assert!(out_setup.contains("[OK] Created default prompt config:"));

    let models_dir = fake_home.join(".local/share/yarube/models");
    assert!(models_dir.is_dir(), "models directory must be created");

    assert!(
        out_setup.contains("[INFO] To download the recommended model and tokenizer manually, run:")
    );
    assert!(out_setup.contains("curl -L -o"));
    assert!(out_setup.contains(HF_MODEL_URL));
    assert!(out_setup.contains(HF_TOKENIZER_URL));

    assert!(!out_setup.contains("❌") && !out_setup.contains("✅") && !out_setup.contains("✨"));

    std::fs::write(models_dir.join("Qwen3-1.7B-Q8_0.gguf"), b"dummy model").unwrap();
    std::fs::write(models_dir.join("tokenizer.json"), b"dummy tokenizer").unwrap();

    let mut buf_already = Vec::new();
    run_setup_to(&mut buf_already, &[]).unwrap();
    let out_already = String::from_utf8(buf_already).unwrap();

    assert!(out_already.contains("[OK] Model and tokenizer are already installed."));

    if let Some(h) = orig_home {
        env::set_var("HOME", h);
    } else {
        env::remove_var("HOME");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
