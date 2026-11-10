use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use yarube_coach::prompt::{
    build_prompt, load_prompt_template, parse_prompt_or_fallback, resolve_external_prompt_path,
    PromptTemplate,
};
use yarube_coach::{
    append_coach_to_log, extract_stream_chunk, load_coach_config, parse_hex_rgb, CoachConfig,
    DEFAULT_COACH_RGB, DEFAULT_EN_PROMPT_TOML, DEFAULT_JA_PROMPT_TOML,
};

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
fn test_default_prompts_parse_and_render() {
    // Japanese default prompt
    let template_ja: PromptTemplate =
        toml::from_str(DEFAULT_JA_PROMPT_TOML).expect("default_ja.toml should parse");
    assert!(!template_ja.examples.is_empty());
    let ja_rendered = template_ja.render("テストする気が起きない。");

    assert!(ja_rendered.contains("<|im_start|>system\n"));
    assert!(ja_rendered.contains("あなたは優しい内省コーチです。"));
    assert!(ja_rendered.contains("/no_think<|im_end|>\n"));
    assert!(ja_rendered.contains(
        "<|im_start|>user\n設計で迷って予定が崩れた。自分はダメなエンジニアだ。<|im_end|>\n"
    ));
    assert!(ja_rendered.contains("<|im_start|>assistant\n<think>\n\n</think>\n\n予定通りに進まず"));
    assert!(ja_rendered.contains(
        "<|im_start|>user\nテストする気が起きない。<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    ));

    // English default prompt
    let template_en: PromptTemplate =
        toml::from_str(DEFAULT_EN_PROMPT_TOML).expect("default_en.toml should parse");
    assert!(!template_en.examples.is_empty());
    let en_rendered = template_en.render("Too tired to work.");

    assert!(en_rendered.contains("<|im_start|>system\n"));
    assert!(en_rendered.contains("You are a gentle reflection coach."));
    assert!(en_rendered.contains("/no_think<|im_end|>\n"));
    assert!(en_rendered.contains(
        "<|im_start|>user\nI got lost in the design and ruined my schedule. I'm a terrible engineer.<|im_end|>\n"
    ));
    assert!(en_rendered
        .contains("<|im_start|>assistant\n<think>\n\n</think>\n\nIt must be so frustrating"));
    assert!(en_rendered.contains(
        "<|im_start|>user\nToo tired to work.<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    ));
}

#[test]
fn test_external_toml_prompt_building() {
    let dir = unique_temp_dir("yarube_coach_ext_toml");
    let custom_file = dir.join("custom_coach.toml");
    let custom_toml = r#"
system = "カスタムコーチ指示"

[[examples]]
user = "1回目の悩み"
assistant = "1回目の返答"

[[examples]]
user = "2回目の悩み"
assistant = "2回目の返答"
"#;
    std::fs::write(&custom_file, custom_toml).unwrap();

    let rendered = build_prompt("今回のふりかえり", false, custom_file.to_str());

    // system automatically gains " /no_think" and ChatML tags
    assert!(rendered.starts_with("<|im_start|>system\nカスタムコーチ指示 /no_think<|im_end|>\n"));
    // First shot with <think>\n\n</think>\n\n
    assert!(rendered.contains(
        "<|im_start|>user\n1回目の悩み<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n1回目の返答<|im_end|>\n"
    ));
    // Second shot with <think>\n\n</think>\n\n
    assert!(rendered.contains(
        "<|im_start|>user\n2回目の悩み<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n2回目の返答<|im_end|>\n"
    ));
    // Current user prompt ending with assistant <think>\n\n</think>\n\n
    assert!(rendered.ends_with(
        "<|im_start|>user\n今回のふりかえり<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_invalid_or_missing_toml_fallback_to_defaults() {
    let dir = unique_temp_dir("yarube_coach_fallback_toml");
    let invalid_file = dir.join("broken.toml");
    std::fs::write(&invalid_file, "system = [unclosed syntax").unwrap();
    let missing_file = dir.join("nonexistent.toml");

    // 1. Invalid TOML string via parse_prompt_or_fallback
    let template_ja = parse_prompt_or_fallback("system = [unclosed syntax", false);
    assert!(template_ja.system.contains("あなたは優しい内省コーチです。"));
    let rendered_ja = template_ja.render("入力");
    assert!(rendered_ja.contains("<think>\n\n</think>\n\n"));
    assert!(rendered_ja.contains("/no_think"));

    let template_en = parse_prompt_or_fallback("system = [unclosed syntax", true);
    assert!(template_en.system.contains("You are a gentle reflection coach."));
    let rendered_en = template_en.render("input");
    assert!(rendered_en.contains("<think>\n\n</think>\n\n"));
    assert!(rendered_en.contains("/no_think"));

    // 2. Invalid external TOML file falls back to default_ja / default_en
    let loaded_broken_ja = load_prompt_template(invalid_file.to_str(), false);
    assert!(loaded_broken_ja.system.contains("あなたは優しい内省コーチです。"));
    let loaded_broken_en = load_prompt_template(invalid_file.to_str(), true);
    assert!(loaded_broken_en
        .system
        .contains("You are a gentle reflection coach."));

    // 3. Missing external TOML file falls back to default_ja / default_en
    let loaded_missing_ja = load_prompt_template(missing_file.to_str(), false);
    assert!(loaded_missing_ja.system.contains("あなたは優しい内省コーチです。"));
    let loaded_missing_en = load_prompt_template(missing_file.to_str(), true);
    assert!(loaded_missing_en
        .system
        .contains("You are a gentle reflection coach."));

    let _ = std::fs::remove_dir_all(&dir);
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
    assert_eq!(default_cfg.max_tokens, 256);
    assert!((default_cfg.temperature - 0.3).abs() < 1e-9);
    assert_eq!(default_cfg.top_k, 10);
    assert!((default_cfg.top_p - 0.8).abs() < 1e-9);
    assert!((default_cfg.repeat_penalty - 1.1).abs() < 1e-6);
    assert_eq!(default_cfg.repeat_last_n, 64);

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
repeat_penalty = 1.2
repeat_last_n = 32
"#,
    )
    .unwrap();

    let custom_cfg = load_coach_config(&custom_path);
    assert!(!custom_cfg.enabled);
    assert_eq!(custom_cfg.model_path, "~/custom/model.gguf");
    assert_eq!(custom_cfg.tokenizer_path, "~/custom/tokenizer.json");
    assert_eq!(
        custom_cfg.prompt_path,
        Some("~/custom/prompt.toml".to_string())
    );
    assert_eq!(custom_cfg.max_tokens, 64);
    assert!((custom_cfg.temperature - 0.5).abs() < 1e-9);
    assert_eq!(custom_cfg.top_k, 20);
    assert!((custom_cfg.top_p - 0.9).abs() < 1e-9);
    assert!((custom_cfg.repeat_penalty - 1.2).abs() < 1e-6);
    assert_eq!(custom_cfg.repeat_last_n, 32);

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

    let generated =
        "  気持ちが向かずモヤモヤしているのですね。\n質問：最小の箇所はどこですか？ \n\n";
    append_coach_to_log(&log_path, generated).unwrap();

    let saved = std::fs::read_to_string(&log_path).unwrap();
    let expected = "### 10/17(火)\n\n#### △ やることリスト\n\n#### △ ふりかえり\nテストする気が起きない。\n> [Coach] 気持ちが向かずモヤモヤしているのですね。\n> 質問：最小の箇所はどこですか？\n";
    assert_eq!(saved, expected);

    let _ = std::fs::remove_dir_all(&dir);
}
