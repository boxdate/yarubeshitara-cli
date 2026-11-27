use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use yarube_coach_ja::cli::{
    parse_cli_args, run_dialect_with_io, run_setup_to, run_status_to, CliCommand,
    HF_PLAMO_MODEL_URL, HF_QWEN_MODEL_URL, HF_QWEN_TOKENIZER_URL,
};
use yarube_coach_ja::load_coach_ja_config;

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
fn test_parse_cli_args_normal_and_dialect_subcommands() {
    let args_none = vec!["yarube-coach-ja".to_string()];
    assert_eq!(parse_cli_args(&args_none), CliCommand::Inference);

    let args_empty: Vec<String> = vec![];
    assert_eq!(parse_cli_args(&args_empty), CliCommand::Inference);

    let args_status = vec!["yarube-coach-ja".to_string(), "status".to_string()];
    assert_eq!(parse_cli_args(&args_status), CliCommand::Status);

    let args_setup = vec!["yarube-coach-ja".to_string(), "setup".to_string()];
    assert_eq!(parse_cli_args(&args_setup), CliCommand::Setup(vec![]));

    let args_setup_no_dl = vec![
        "yarube-coach-ja".to_string(),
        "setup".to_string(),
        "--no-download".to_string(),
    ];
    assert_eq!(
        parse_cli_args(&args_setup_no_dl),
        CliCommand::Setup(vec!["--no-download".to_string()])
    );

    let args_dialect_interactive = vec!["yarube-coach-ja".to_string(), "dialect".to_string()];
    assert_eq!(
        parse_cli_args(&args_dialect_interactive),
        CliCommand::Dialect(None)
    );

    let args_dialect_direct = vec![
        "yarube-coach-ja".to_string(),
        "dialect".to_string(),
        "hokkaido_donan".to_string(),
    ];
    assert_eq!(
        parse_cli_args(&args_dialect_direct),
        CliCommand::Dialect(Some("hokkaido_donan".to_string()))
    );

    for flag in ["--help", "-h", "help"] {
        let args_help = vec!["yarube-coach-ja".to_string(), flag.to_string()];
        assert_eq!(parse_cli_args(&args_help), CliCommand::Help);
    }

    let args_unknown = vec!["yarube-coach-ja".to_string(), "unknown".to_string()];
    assert_eq!(parse_cli_args(&args_unknown), CliCommand::Status);
}

#[test]
fn test_status_and_setup_with_dialects_and_models() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = unique_temp_dir("yarube_coach_ja_status_test");
    let fake_home = dir.join("home");
    std::fs::create_dir_all(&fake_home).unwrap();

    let orig_home = env::var("HOME").ok();
    let orig_cfg = env::var("YARUBE_CONFIG_PATH").ok();
    env::set_var("HOME", &fake_home);
    env::remove_var("YARUBE_CONFIG_PATH");

    // 1. Initial status (missing models, built-in dialect)
    let mut buf_missing = Vec::new();
    run_status_to(&mut buf_missing).unwrap();
    let out_missing = String::from_utf8(buf_missing).unwrap();

    assert!(out_missing.contains("[yarube-coach-ja status]"));
    assert!(out_missing.contains("- PLaMo 3 model  :"));
    assert!(out_missing.contains("- Qwen3 model    :"));
    assert!(out_missing.contains("- Qwen3 tokenizer:"));
    assert!(out_missing.contains("[MISSING]"));
    assert!(out_missing.contains("- Active dialect : 北海道・道南弁（やるべしたらモード）"));
    assert!(out_missing.contains("[BUILT-IN]"));
    assert!(out_missing.contains("- Memory policy  : sequential_drop"));
    assert!(!out_missing.contains("❌") && !out_missing.contains("✅"));

    // 2. Setup with --no-download creates dialect JSONs and default prompt TOML
    let mut buf_setup = Vec::new();
    run_setup_to(&mut buf_setup, &["--no-download".to_string()]).unwrap();
    let out_setup = String::from_utf8(buf_setup).unwrap();

    let dialects_dir = fake_home.join(".config/yarube/dialects");
    assert!(dialects_dir.join("standard_polite.json").is_file());
    assert!(dialects_dir.join("hokkaido_donan.json").is_file());
    assert!(dialects_dir.join("kansai.json").is_file());
    assert!(fake_home.join(".config/yarube/coach_ja_prompt.toml").is_file());

    assert!(out_setup.contains(HF_PLAMO_MODEL_URL));
    assert!(out_setup.contains(HF_QWEN_MODEL_URL));
    assert!(out_setup.contains(HF_QWEN_TOKENIZER_URL));

    // 3. Place dummy model files and verify status becomes [OK]
    let models_dir = fake_home.join(".local/share/yarube/models");
    std::fs::write(
        models_dir.join("plamo-3-translate-0.8b-Q4_K_M.gguf"),
        b"dummy plamo",
    )
    .unwrap();
    std::fs::write(models_dir.join("Qwen3-1.7B-Q8_0.gguf"), b"dummy qwen").unwrap();
    std::fs::write(models_dir.join("tokenizer.json"), b"dummy tok").unwrap();

    let mut buf_ok = Vec::new();
    run_status_to(&mut buf_ok).unwrap();
    let out_ok = String::from_utf8(buf_ok).unwrap();
    assert!(!out_ok.contains("[MISSING]"));
    assert!(out_ok.contains("[OK]"));
    assert!(out_ok.contains("[JSON]"));
    assert!(out_ok.contains("[CUSTOM]"));

    if let Some(h) = orig_home {
        env::set_var("HOME", h);
    } else {
        env::remove_var("HOME");
    }
    if let Some(c) = orig_cfg {
        env::set_var("YARUBE_CONFIG_PATH", c);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_dialect_subcommand_direct_and_interactive_switching() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = unique_temp_dir("yarube_coach_ja_dialect_cmd");
    let fake_home = dir.join("home");
    std::fs::create_dir_all(&fake_home).unwrap();
    let cfg_path = fake_home.join(".config/yarube/config.toml");

    let orig_home = env::var("HOME").ok();
    let orig_cfg = env::var("YARUBE_CONFIG_PATH").ok();
    env::set_var("HOME", &fake_home);
    env::set_var("YARUBE_CONFIG_PATH", &cfg_path);

    // 1. Direct switch to `kansai`
    let mut empty_in = Cursor::new(b"" as &[u8]);
    let mut out1 = Vec::new();
    run_dialect_with_io(Some("kansai"), &mut empty_in, &mut out1).unwrap();
    let out1_str = String::from_utf8(out1).unwrap();
    assert!(out1_str.contains("kansai"));
    assert!(out1_str.contains("関西弁"));

    let loaded1 = load_coach_ja_config(&cfg_path);
    assert_eq!(loaded1.dialect, "kansai");

    // 2. Interactive switch by number `1` (`standard_polite`)
    let mut num_in = Cursor::new(b"1\n" as &[u8]);
    let mut out2 = Vec::new();
    run_dialect_with_io(None, &mut num_in, &mut out2).unwrap();
    let out2_str = String::from_utf8(out2).unwrap();
    assert!(out2_str.contains("[yarube-coach-ja dialect]"));
    assert!(out2_str.contains("standard_polite"));
    assert!(out2_str.contains("標準語（です・ます調）"));

    let loaded2 = load_coach_ja_config(&cfg_path);
    assert_eq!(loaded2.dialect, "standard_polite");

    // 3. Interactive switch by number `2` (`hokkaido_donan`)
    let mut num_in2 = Cursor::new(b"2\n" as &[u8]);
    let mut out3 = Vec::new();
    run_dialect_with_io(None, &mut num_in2, &mut out3).unwrap();
    let loaded3 = load_coach_ja_config(&cfg_path);
    assert_eq!(loaded3.dialect, "hokkaido_donan");

    if let Some(h) = orig_home {
        env::set_var("HOME", h);
    } else {
        env::remove_var("HOME");
    }
    if let Some(c) = orig_cfg {
        env::set_var("YARUBE_CONFIG_PATH", c);
    } else {
        env::remove_var("YARUBE_CONFIG_PATH");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
