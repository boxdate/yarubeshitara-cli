use candle_plamo3::pfml3;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use yarube_coach_ja::{
    append_coach_to_log, build_en_coaching_prompt, extract_stream_chunk, load_coach_ja_config,
    resolve_dialect_by_spec, trim_plamo_special_suffix, upsert_coach_ja_dialect_in_toml,
    CoachJaConfig, PromptTemplate, DEFAULT_EN_PROMPT_TOML,
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
fn test_three_step_prompts_ja_en_coaching_and_dialect_pfml3() {
    // Step 1: JA -> EN PFML3 prompt
    let user_ja = "Sakutto使ってみた！ただ設計で迷って予定が崩れた。";
    let step1_prompt = pfml3::prompt_ja_to_en(user_ja);
    assert_eq!(
        step1_prompt,
        format!(
            "<|plamo:tag|>dataset<|plamo:msg|>translation\
             <|plamo:tag|>input<|plamo:key|>lang<|plamo:val|>ja<|plamo:msg|>{user_ja}\
             <|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>en<|plamo:msg|>"
        )
    );

    // Step 2: EN Coaching ChatML + /no_think prompt
    let template_en: PromptTemplate =
        toml::from_str(DEFAULT_EN_PROMPT_TOML).expect("default_en.toml should parse");
    assert!(!template_en.examples.is_empty());
    let en_memo = "I tried Sakutto! However, I hesitated over the design and fell behind schedule.";
    let step2_prompt = build_en_coaching_prompt(en_memo, Some("/nonexistent/prompt.toml"));
    assert!(step2_prompt.contains("<|im_start|>system\n"));
    assert!(step2_prompt.contains("You are a warm and supportive reflection coach."));
    assert!(step2_prompt.contains("/no_think<|im_end|>\n"));
    assert!(step2_prompt.contains("<think>\n\n</think>\n\n"));
    assert!(step2_prompt.ends_with(&format!(
        "<|im_start|>user\n{en_memo}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
    )));

    // Step 3: EN -> JA with DialectConfig (Hokkaido Donan, Standard Polite, Kansai)
    let en_coaching = "It's great to hear you were impressed by Sakutto, even though the schedule delay was frustrating.\nQuestion: How many specific places did you get stuck on today?";

    let donan = resolve_dialect_by_spec("hokkaido_donan", "/nonexistent/dialects");
    assert_eq!(donan.id, "hokkaido_donan");
    assert_eq!(donan.config.form, None);
    let step3_donan_prompt = pfml3::prompt_en_to_ja_with_config(en_coaching, &donan.config);
    assert!(step3_donan_prompt.contains("<|plamo:tag|>context<|plamo:msg|>北海道の道南地方"));
    assert!(step3_donan_prompt
        .contains("<|plamo:index|>Question:<|plamo:item|>ちょっと聞くけどさ："));
    assert!(step3_donan_prompt.contains("<|plamo:index|>Sakutto<|plamo:item|>Sakutto"));
    assert!(step3_donan_prompt.contains("<|plamo:index|>Bullet<|plamo:item|>Bullet"));
    // form is None for dialect so no `<|plamo:key|>form` tag should be emitted
    assert!(step3_donan_prompt
        .ends_with("<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>ja<|plamo:msg|>"));

    let polite = resolve_dialect_by_spec("standard_polite", "/nonexistent/dialects");
    assert_eq!(polite.id, "standard_polite");
    assert_eq!(polite.config.form.as_deref(), Some("polite"));
    let step3_polite_prompt = pfml3::prompt_en_to_ja_with_config(en_coaching, &polite.config);
    assert!(step3_polite_prompt.contains("<|plamo:index|>Question:<|plamo:item|>質問："));
    assert!(step3_polite_prompt.contains("<|plamo:index|>Sakutto<|plamo:item|>Sakutto"));
    assert!(step3_polite_prompt.ends_with(
        "<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>ja<|plamo:key|>form<|plamo:val|>polite<|plamo:msg|>"
    ));

    let kansai = resolve_dialect_by_spec("kansai", "/nonexistent/dialects");
    assert_eq!(kansai.id, "kansai");
    let step3_kansai_prompt = pfml3::prompt_en_to_ja_with_config(en_coaching, &kansai.config);
    assert!(step3_kansai_prompt
        .contains("<|plamo:index|>Question:<|plamo:item|>ほんで1つ聞きたいんやけど："));
}

#[test]
fn test_custom_external_dialect_json_loading() {
    let dir = unique_temp_dir("yarube_coach_ja_custom_dialect");
    let custom_json_path = dir.join("tsugaru.json");
    std::fs::write(
        &custom_json_path,
        r#"{
  "name": "津軽弁モード",
  "form": null,
  "context": "青森の津軽弁で温かく翻訳してください。",
  "glossary": [
    { "source": "Question:", "target": "ひとつ聞くばって：" }
  ],
  "few_shots": []
}"#,
    )
    .unwrap();

    // Load by direct file path
    let resolved_direct =
        resolve_dialect_by_spec(custom_json_path.to_str().unwrap(), dir.to_str().unwrap());
    assert_eq!(resolved_direct.id, "tsugaru");
    assert_eq!(resolved_direct.config.name, "津軽弁モード");
    assert!(!resolved_direct.is_builtin_fallback);

    // Load by stem name inside dialects_dir
    let resolved_stem = resolve_dialect_by_spec("tsugaru", dir.to_str().unwrap());
    assert_eq!(resolved_stem.id, "tsugaru");
    assert_eq!(resolved_stem.config.name, "津軽弁モード");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_config_parsing_and_dialect_upsert() {
    let dir = unique_temp_dir("yarube_coach_ja_cfg_test");
    let cfg_path = dir.join("config.toml");

    let initial_toml = r#"
[general]
theme = "tokyo-night"

[plugins]
on_reflect = ["coach-ja"]

[plugins.coach-ja]
enabled = true
dialect = "standard_polite"
keep_plamo_in_memory = true
step1_max_tokens = 200
step2_max_tokens = 100
step3_max_tokens = 180
"#;
    std::fs::write(&cfg_path, initial_toml).unwrap();

    let cfg = load_coach_ja_config(&cfg_path);
    assert!(cfg.enabled);
    assert_eq!(cfg.dialect, "standard_polite");
    assert!(cfg.keep_plamo_in_memory);
    assert_eq!(cfg.step1_max_tokens, 200);
    assert_eq!(cfg.step2_max_tokens, 100);
    assert_eq!(cfg.step3_max_tokens, 180);

    // Upsert dialect while preserving other keys
    let updated_toml = upsert_coach_ja_dialect_in_toml(initial_toml, "hokkaido_donan");
    std::fs::write(&cfg_path, &updated_toml).unwrap();
    let cfg_updated = load_coach_ja_config(&cfg_path);
    assert_eq!(cfg_updated.dialect, "hokkaido_donan");
    assert!(cfg_updated.keep_plamo_in_memory);
    assert_eq!(cfg_updated.step1_max_tokens, 200);

    // Default config check
    let default_cfg = CoachJaConfig::default();
    assert_eq!(default_cfg.dialect, "hokkaido_donan");
    assert!(!default_cfg.keep_plamo_in_memory);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_plamo_special_suffix_trimming_and_multibyte_stream() {
    let raw_out = "予定通りにいかなかったのは、なまら悔しかったべさ。\nちょっと聞くけどさ：今日どこで手が止まったんかい？<|plamo:tag|>input";
    let trimmed = trim_plamo_special_suffix(raw_out);
    assert_eq!(
        trimmed,
        "予定通りにいかなかったのは、なまら悔しかったべさ。\nちょっと聞くけどさ：今日どこで手が止まったんかい？"
    );

    let mut printed_len = 0;
    assert_eq!(
        extract_stream_chunk("なまら\u{FFFD}", &mut printed_len),
        None
    );
    let chunk = extract_stream_chunk(
        "なまら悔しかったべさ。\nちょっと聞くけどさ：",
        &mut printed_len,
    )
    .unwrap();
    assert_eq!(chunk, "なまら悔しかったべさ。\n> ちょっと聞くけどさ：");

    let dir = unique_temp_dir("yarube_coach_ja_log");
    let log_path = dir.join("2026-10-17.md");
    let mut f = File::create(&log_path).unwrap();
    f.write_all(b"### 10/17(\xE5\x9C\x9F)\n\n#### \xE2\x96\xB3 \xE3\x81\xB5\xE3\x82\x8A\xE3\x81\x8B\xE3\x81\x88\xE3\x82\x8A\n\xE9\x9B\xA3\xE5\x84\x80\xE3\x81\x97\xE3\x81\x9F\xE3\x80\x82\n")
        .unwrap();

    append_coach_to_log(&log_path, &trimmed).unwrap();
    let saved = std::fs::read_to_string(&log_path).unwrap();
    assert!(saved.contains("> [Coach] 予定通りにいかなかったのは、なまら悔しかったべさ。\n> ちょっと聞くけどさ：今日どこで手が止まったんかい？\n"));

    let _ = std::fs::remove_dir_all(&dir);
}
