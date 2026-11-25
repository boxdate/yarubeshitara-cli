use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestEnv {
    dir: PathBuf,
    date: String,
}

impl TestEnv {
    fn new(date: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = env::temp_dir().join(format!("yarube_test_{}_{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create test temp dir");
        Self {
            dir,
            date: date.to_string(),
        }
    }

    fn bin_path() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_yarube"))
    }

    fn cmd(&self) -> Command {
        self.cmd_on_date(&self.date)
    }

    fn cmd_on_date(&self, date: &str) -> Command {
        let mut c = Command::new(Self::bin_path());
        c.env("YARUBE_DIR", &self.dir);
        c.env("YARUBE_DATE", date);
        c.env_remove("BUJO_DIR");
        c.env_remove("BUJO_DATE");
        c.env_remove("YARUBE_LANG");
        c.env_remove("BUJO_LANG");
        c.env_remove("YARUBE_THEME");
        c.env_remove("BUJO_THEME");
        c.env_remove("LC_ALL");
        c.env_remove("LC_MESSAGES");
        c.env("LANG", "ja_JP.UTF-8");
        c
    }

    fn log_path(&self, date: &str) -> PathBuf {
        self.dir.join(format!("{}.md", date))
    }

    fn config_path(&self) -> PathBuf {
        self.dir.join("config")
    }

    fn read_log(&self, date: &str) -> String {
        fs::read_to_string(self.log_path(date)).expect("read daily log file")
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// [TP-01 / BDD 01-1 / EARS-01]
#[test]
fn tp01_add_task_explicit_symbol_and_performance_under_100ms() {
    let env = TestEnv::new("2026-10-17");
    let _ = env.cmd().args(["t", "ウォームアップ"]).output().unwrap();

    let env_perf = TestEnv::new("2026-10-17");
    let start = Instant::now();
    let output = env_perf
        .cmd()
        .args(["t", "SIT-1-1-1-001のテスト作る"])
        .output()
        .expect("execute bujo t");
    let elapsed = start.elapsed();

    assert!(output.status.success(), "bujo t should exit with code 0");
    assert!(
        elapsed < Duration::from_millis(100),
        "bujo t should finish within 0.1s (100ms), took {:?}",
        elapsed
    );

    let saved = env_perf.read_log("2026-10-17");
    assert!(saved.contains("### 10/17(土)"));
    assert!(saved.contains("#### △ やることリスト\n• SIT-1-1-1-001のテスト作る\n"));
    assert!(saved.contains("#### △ ふりかえり\n"));
}

/// [TP-02 / BDD 01-2 / EARS-02]
#[test]
fn tp02_add_subtask_indented_under_preceding_task() {
    let env = TestEnv::new("2026-10-17");

    env.cmd()
        .args(["t", "PodmanにGitbucketを立ち上げる"])
        .output()
        .unwrap();
    let out = env
        .cmd()
        .args(["sub", "Lubuntu -> Xubuntuが良いか"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-17");
    let expected_section =
        "#### △ やることリスト\n• PodmanにGitbucketを立ち上げる\n  - Lubuntu -> Xubuntuが良いか\n";
    assert!(
        saved.contains(expected_section),
        "Expected indented subtask under parent task, got:\n{}",
        saved
    );
}

/// [TP-03 / BDD 01-3 / EARS-03]
#[test]
fn tp03_default_symbol_omitted_adds_task() {
    let env = TestEnv::new("2026-10-17");

    let out = env
        .cmd()
        .args(["MCP用の変換ツール作成する"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-17");
    assert!(saved.contains("#### △ やることリスト\n• MCP用の変換ツール作成する\n"));
}

/// [TP-04 / BDD 01-4 / EARS-04]
#[test]
fn tp04_unquoted_multiple_arguments_joined_with_spaces() {
    let env = TestEnv::new("2026-10-17");

    let out = env
        .cmd()
        .args(["明日の", "会議", "資料作成"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-17");
    assert!(saved.contains("• 明日の 会議 資料作成\n"));
}

/// [TP-05 / BDD 01-5 / EARS-05]
#[test]
fn tp05_display_existing_daily_log_with_no_args() {
    let env = TestEnv::new("2026-10-17");
    env.cmd().args(["t", "既存タスク1"]).output().unwrap();
    env.cmd().args(["reflect", "既存のふりかえり"]).output().unwrap();

    let out = env.cmd().output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("### 10/17(土)"));
    assert!(stdout.contains("#### △ やることリスト"));
    assert!(stdout.contains("• 既存タスク1"));
    assert!(stdout.contains("#### △ ふりかえり"));
    assert!(stdout.contains("既存のふりかえり"));
}

/// [TP-06 / BDD 01-6 / EARS-06]
#[test]
fn tp06_auto_generate_template_and_show_guide_on_new_day() {
    let env = TestEnv::new("2026-10-20");
    assert!(!env.log_path("2026-10-20").exists());

    let out = env.cmd().output().unwrap();
    assert!(out.status.success());

    assert!(env.log_path("2026-10-20").exists());
    let saved = env.read_log("2026-10-20");
    assert!(saved.contains("### 10/20(火)"));
    assert!(saved.contains("#### △ やることリスト"));
    assert!(saved.contains("#### △ ふりかえり"));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("今日の手帳を用意しました"));
    assert!(stdout.contains("yarube タスク内容"));
}

/// [TP-07 / BDD 01-7 / EARS-07]
#[test]
fn tp07_redraw_entire_log_and_highlight_newly_added_line() {
    let env = TestEnv::new("2026-10-20");
    env.cmd().args(["t", "最初のタスク"]).output().unwrap();

    let out = env.cmd().args(["t", "2番目の追加タスク"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("#### △ やることリスト"));
    assert!(stdout.contains("• 最初のタスク"));
    assert!(stdout.contains("#### △ ふりかえり"));

    assert!(
        !stdout.contains("• 最初のタスク  [NEW]"),
        "Previous task should NOT be highlighted as NEW"
    );
    assert!(
        stdout.contains("• 2番目の追加タスク  [NEW]"),
        "Newly added task MUST be highlighted as NEW, stdout:\n{}",
        stdout
    );
}

/// [TP-08 / BDD 02-1 / EARS-08]
#[test]
fn tp08_interactive_review_updates_task_statuses() {
    let env = TestEnv::new("2026-10-20");
    env.cmd().args(["t", "完了にするタスク"]).output().unwrap();
    env.cmd().args(["t", "移行するタスク"]).output().unwrap();
    env.cmd().args(["t", "取り消すタスク"]).output().unwrap();
    env.cmd().args(["t", "スキップするタスク"]).output().unwrap();

    let mut child = env
        .cmd()
        .arg("review")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"o>cs").unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-20");
    assert!(saved.contains("O 完了にするタスク\n"), "saved:\n{}", saved);
    assert!(saved.contains("> 移行するタスク\n"), "saved:\n{}", saved);
    assert!(saved.contains("~ 取り消すタスク\n"), "saved:\n{}", saved);
    assert!(saved.contains("• スキップするタスク\n"), "saved:\n{}", saved);
}

/// [TP-09 / BDD 02-2 / EARS-09]
#[test]
fn tp09_review_interruption_preserves_confirmed_updates() {
    let env = TestEnv::new("2026-10-20");
    env.cmd().args(["t", "タスク1"]).output().unwrap();
    env.cmd().args(["t", "タスク2"]).output().unwrap();
    env.cmd().args(["t", "タスク3"]).output().unwrap();

    let mut child = env
        .cmd()
        .arg("review")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"o>\x03").unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-20");
    assert!(saved.contains("O タスク1\n"), "Task 1 should be saved as O:\n{}", saved);
    assert!(saved.contains("> タスク2\n"), "Task 2 should be saved as >:\n{}", saved);
    assert!(saved.contains("• タスク3\n"), "Task 3 should remain •:\n{}", saved);
}

/// [TP-10 / BDD 02-3 / EARS-10]
#[test]
fn tp10_migrate_task_with_subtasks_to_next_day_log() {
    let env = TestEnv::new("2026-10-17");
    env.cmd()
        .args(["t", "PodmanにGitbucketを立ち上げる"])
        .output()
        .unwrap();
    env.cmd()
        .args(["sub", "Lubuntu -> Xubuntuが良いか"])
        .output()
        .unwrap();

    let mut child = env
        .cmd()
        .arg("review")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b">").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());

    let today_log = env.read_log("2026-10-17");
    assert!(
        today_log.contains("> PodmanにGitbucketを立ち上げる\n  - Lubuntu -> Xubuntuが良いか\n"),
        "Today's log:\n{}",
        today_log
    );

    let tomorrow_log = env.read_log("2026-10-18");
    assert!(tomorrow_log.contains("### 10/18(日)"));
    assert!(
        tomorrow_log.contains("#### △ やることリスト\n• PodmanにGitbucketを立ち上げる\n  - Lubuntu -> Xubuntuが良いか\n"),
        "Tomorrow's log:\n{}",
        tomorrow_log
    );
}

/// [TP-11 / BDD 02-4 / EARS-11]
#[test]
fn tp11_reflect_via_editor_dumps_multiline_text_to_reflection_section() {
    let env = TestEnv::new("2026-10-17");

    let editor_script = env.dir.join("mock_editor.sh");
    fs::write(
        &editor_script,
        "#!/bin/sh\ncat << 'EOF' > \"$1\"\nEndeavourOSにしたあき！\nUXって大事。本は格段に読みやすくなったので、勉強頑張ろう。\nEOF\n",
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", editor_script.to_str().unwrap()])
        .status();

    let out = env
        .cmd()
        .env_remove("VISUAL")
        .env("EDITOR", editor_script.to_str().unwrap())
        .arg("reflect")
        .output()
        .unwrap();
    assert!(out.status.success());

    let saved = env.read_log("2026-10-17");
    assert!(
        saved.contains("#### △ ふりかえり\nEndeavourOSにしたあき！\nUXって大事。本は格段に読みやすくなったので、勉強頑張ろう。\n"),
        "Saved log:\n{}",
        saved
    );
}

/// [TP-12 / BDD 03-1 / EARS-12]
/// Given: 設定でエディタ(code --wait相当)を指定し、環境変数($VISUAL/$EDITOR)に別コマンドが設定されている
/// When: bujo reflect を実行
/// Then: 設定ファイルのエディタが環境変数より優先して起動し、保存終了まで待機して反映される
#[test]
fn tp12_editor_config_priority_over_visual_and_editor_env_vars() {
    let env = TestEnv::new("2026-10-20");

    // Mock "code" script that accepts "--wait" and writes to the target file ($2)
    let config_editor_script = env.dir.join("mock_code.sh");
    fs::write(
        &config_editor_script,
        "#!/bin/sh\nif [ \"$1\" = \"--wait\" ]; then\n  echo \"VS Code(--wait)から保存されたふりかえり\" > \"$2\"\nfi\n",
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", config_editor_script.to_str().unwrap()])
        .status();

    // Mock env editor that should NOT be called when config is set
    let env_editor_script = env.dir.join("mock_env_editor.sh");
    fs::write(
        &env_editor_script,
        "#!/bin/sh\necho \"ENV_EDITORから保存されたふりかえり\" > \"$1\"\n",
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", env_editor_script.to_str().unwrap()])
        .status();

    // Set config editor with "--wait" flag
    let cfg_cmd = format!("{} --wait", config_editor_script.to_str().unwrap());
    let cfg_out = env
        .cmd()
        .args(["config", "editor", &cfg_cmd])
        .output()
        .unwrap();
    assert!(cfg_out.status.success());

    // Run `bujo reflect` with both VISUAL and EDITOR set to `env_editor_script`
    let reflect_out = env
        .cmd()
        .env("VISUAL", env_editor_script.to_str().unwrap())
        .env("EDITOR", env_editor_script.to_str().unwrap())
        .arg("reflect")
        .output()
        .unwrap();
    assert!(reflect_out.status.success());

    let saved = env.read_log("2026-10-20");
    assert!(
        saved.contains("VS Code(--wait)から保存されたふりかえり"),
        "Config editor should take priority over VISUAL/EDITOR:\n{}",
        saved
    );
    assert!(!saved.contains("ENV_EDITORから保存されたふりかえり"));
}

/// [TP-13 / BDD 03-2 / EARS-13]
/// Given: ターミナルが開いている
/// When: bujo config editor を実行しリストから選択（例: 4 = code --wait）
/// Then: 設定ファイルが更新され永続化される
#[test]
fn tp13_interactive_config_editor_selection_persists_to_config_file() {
    let env = TestEnv::new("2026-10-20");

    let mut child = env
        .cmd()
        .args(["config", "editor"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        // Select option 4 ("code --wait")
        stdin.write_all(b"4\n").unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("code --wait"));

    let config_content = fs::read_to_string(env.config_path()).expect("read config file");
    assert!(
        config_content.contains("editor = \"code --wait\""),
        "Config file content:\n{}",
        config_content
    );
}

/// [TP-14 / BDD 04-1 / EARS-14]
/// Given: 過去ログが存在する（例: 金曜日のログがあり、月曜日に実行）
/// When: bujo prev や bujo -1 を実行
/// Then: 直前の記録日（1営業日前）の手帳全体が標準出力へ整形表示される
#[test]
fn tp14_relative_navigation_prev_and_minus_one_shows_previous_recorded_day() {
    let env = TestEnv::new("2026-10-19"); // Monday

    // Record a log on Friday 2026-10-16 (skipping Saturday 10/17 and Sunday 10/18)
    env.cmd_on_date("2026-10-16")
        .args(["t", "金曜日のタスク"])
        .output()
        .unwrap();
    env.cmd_on_date("2026-10-16")
        .args(["reflect", "金曜日のふりかえりログ"])
        .output()
        .unwrap();

    // Record today's log on Monday 2026-10-19
    env.cmd_on_date("2026-10-19")
        .args(["t", "月曜日のタスク"])
        .output()
        .unwrap();

    // Test `bujo prev`
    let out_prev = env.cmd_on_date("2026-10-19").arg("prev").output().unwrap();
    assert!(out_prev.status.success());
    let stdout_prev = String::from_utf8_lossy(&out_prev.stdout);
    assert!(stdout_prev.contains("### 10/16(金)"));
    assert!(stdout_prev.contains("• 金曜日のタスク"));
    assert!(stdout_prev.contains("金曜日のふりかえりログ"));

    // Test `bujo -1`
    let out_m1 = env.cmd_on_date("2026-10-19").arg("-1").output().unwrap();
    assert!(out_m1.status.success());
    let stdout_m1 = String::from_utf8_lossy(&out_m1.stdout);
    assert!(stdout_m1.contains("### 10/16(金)"));
    assert!(stdout_m1.contains("• 金曜日のタスク"));

    // Test `bujo next` / `bujo +1` from Friday 2026-10-16 jumps to Monday 2026-10-19
    let out_next = env.cmd_on_date("2026-10-16").arg("next").output().unwrap();
    assert!(out_next.status.success());
    let stdout_next = String::from_utf8_lossy(&out_next.stdout);
    assert!(stdout_next.contains("### 10/19(月)"));
    assert!(stdout_next.contains("• 月曜日のタスク"));
}

/// [TP-15 / BDD 04-2 / EARS-15]
/// Given: 過去ログファイル群が存在する
/// When: bujo search 'EndeavourOS' を実行
/// Then: ヒットした日付と該当行（およびタスクブロック）がハイライト付きで抽出表示される
#[test]
fn tp15_full_text_search_extracts_matching_dates_and_blocks_with_highlight() {
    let env = TestEnv::new("2026-10-20");

    // Day 1: 2026-10-17 (has EndeavourOS in reflection and Xubuntu in subtask)
    env.cmd_on_date("2026-10-17")
        .args(["t", "PodmanにGitbucketを立ち上げる"])
        .output()
        .unwrap();
    env.cmd_on_date("2026-10-17")
        .args(["sub", "Lubuntu -> Xubuntuが良いか"])
        .output()
        .unwrap();
    env.cmd_on_date("2026-10-17")
        .args([
            "reflect",
            "EndeavourOSにしたあき！UXって大事。",
        ])
        .output()
        .unwrap();

    // Day 2: 2026-10-18 (unrelated log)
    env.cmd_on_date("2026-10-18")
        .args(["t", "牛乳を買う"])
        .output()
        .unwrap();

    // Search for "EndeavourOS"
    let out = env.cmd().args(["search", "EndeavourOS"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("2026-10-17"), "Should show matched date:\n{}", stdout);
    assert!(stdout.contains("### 10/17(土)"), "Should show matched header:\n{}", stdout);
    assert!(stdout.contains("EndeavourOS"), "Should contain matched keyword:\n{}", stdout);
    assert!(stdout.contains("にしたあき！"), "Should contain matched reflection line:\n{}", stdout);
    assert!(!stdout.contains("2026-10-18"), "Unrelated date should not be shown:\n{}", stdout);

    // Search for subtask keyword "Xubuntu" should extract parent task + subtask block
    let out_sub = env.cmd().args(["search", "Xubuntu"]).output().unwrap();
    assert!(out_sub.status.success());
    let stdout_sub = String::from_utf8_lossy(&out_sub.stdout);
    assert!(stdout_sub.contains("2026-10-17"));
    assert!(stdout_sub.contains("PodmanにGitbucketを立ち上げる"));
    assert!(stdout_sub.contains("Lubuntu -> "));
}

/// [TP-16 / BDD 05-1 / EARS-16]
/// Given: LANG=en_USのターミナル / When: bujo review を未完了0件で実行
/// Then: 'No pending tasks found!' と英語でねぎらいが表示される（LANG=ja_JPでは日本語メッセージ）
#[test]
fn tp16_locale_auto_detection_switches_ui_messages_and_headers_between_en_and_ja() {
    // 1. English locale (LANG=en_US.UTF-8)
    let env_en = TestEnv::new("2026-10-21");
    let out_en_review = env_en
        .cmd()
        .env("LANG", "en_US.UTF-8")
        .arg("review")
        .output()
        .unwrap();
    assert!(out_en_review.status.success());
    let stdout_en = String::from_utf8_lossy(&out_en_review.stdout);
    assert!(
        stdout_en.contains("No pending tasks found!"),
        "Expected English message 'No pending tasks found!', got:\n{}",
        stdout_en
    );

    // Also verify initial template headers generated in English locale
    let saved_en = env_en.read_log("2026-10-21");
    assert!(saved_en.contains("### 10/21(Wed)"), "saved_en:\n{}", saved_en);
    assert!(saved_en.contains("#### △ Todo List"), "saved_en:\n{}", saved_en);
    assert!(saved_en.contains("#### △ Reflection"), "saved_en:\n{}", saved_en);

    // 2. Japanese locale (LANG=ja_JP.UTF-8)
    let env_ja = TestEnv::new("2026-10-21");
    let out_ja_review = env_ja
        .cmd()
        .env("LANG", "ja_JP.UTF-8")
        .arg("review")
        .output()
        .unwrap();
    assert!(out_ja_review.status.success());
    let stdout_ja = String::from_utf8_lossy(&out_ja_review.stdout);
    assert!(
        stdout_ja.contains("未完了のタスクはありません"),
        "Expected Japanese message, got:\n{}",
        stdout_ja
    );
}

/// [TP-17 / BDD 05-2 / EARS-17]
/// Given: 英語ヘッダーの手帳Markdown(Todo List / Reflection)が存在する
/// When: 日本語ロケール環境で bujo を実行
/// Then: 壊れることなくタスクとふりかえりがパースされて整形出力される
#[test]
fn tp17_bilingual_parser_recognizes_both_english_and_japanese_headings() {
    let env = TestEnv::new("2026-10-21");

    // First, create a daily log in English locale (with '#### △ Todo List' and '#### △ Reflection')
    env.cmd()
        .env("LANG", "en_US.UTF-8")
        .args(["t", "English header task"])
        .output()
        .unwrap();
    env.cmd()
        .env("LANG", "en_US.UTF-8")
        .args(["sub", "English subtask detail"])
        .output()
        .unwrap();
    env.cmd()
        .env("LANG", "en_US.UTF-8")
        .args(["reflect", "Reflection written under English heading."])
        .output()
        .unwrap();

    let raw_md = env.read_log("2026-10-21");
    assert!(raw_md.contains("#### △ Todo List"));
    assert!(raw_md.contains("#### △ Reflection"));

    // Now run `bujo` in Japanese locale (LANG=ja_JP.UTF-8) on the same file
    let out_ja = env
        .cmd()
        .env("LANG", "ja_JP.UTF-8")
        .output()
        .unwrap();
    assert!(out_ja.status.success());
    let stdout_ja = String::from_utf8_lossy(&out_ja.stdout);

    assert!(
        stdout_ja.contains("### 10/21(水)"),
        "Header weekday must be localized to Japanese (水) when viewed in ja_JP:\n{}",
        stdout_ja
    );
    assert!(
        stdout_ja.contains("• English header task"),
        "Task under 'Todo List' must be parsed in Japanese locale:\n{}",
        stdout_ja
    );
    assert!(
        stdout_ja.contains("  - English subtask detail"),
        "Subtask must be preserved:\n{}",
        stdout_ja
    );
    assert!(
        stdout_ja.contains("Reflection written under English heading."),
        "Reflection under 'Reflection' must be parsed in Japanese locale:\n{}",
        stdout_ja
    );

    // Conversely, overwrite file with Japanese header `### 10/21(水)` and view in English locale
    env.cmd()
        .env("LANG", "ja_JP.UTF-8")
        .args(["t", "日本語環境で追加したタスク"])
        .output()
        .unwrap();
    let out_en = env
        .cmd()
        .env("LANG", "en_US.UTF-8")
        .output()
        .unwrap();
    assert!(out_en.status.success());
    let stdout_en = String::from_utf8_lossy(&out_en.stdout);
    assert!(
        stdout_en.contains("### 10/21(Wed)"),
        "Header weekday must be localized to English (Wed) even if file had (水):\n{}",
        stdout_en
    );
    assert!(
        !stdout_en.contains("### 10/21(水)"),
        "Should not display Kanji weekday (水) in English mode:\n{}",
        stdout_en
    );
}

/// [TP-18 / BDD 06-1 / EARS-18]
/// Given: theme='monokai' が設定されている / When: bujo を実行
/// Then: 日付が#66d9ef(Bold)、見出しが#fd971f(Bold)、完了タスクが#75715e(Dim+取消線)で描画される
#[test]
fn tp18_theme_monokai_applies_true_color_hex_and_text_decorations() {
    let env = TestEnv::new("2026-10-21");

    // Prepare tasks with Done(O), Migrated(>), Todo(•), and subtask(  - )
    env.cmd().args(["t", "完了したタスク"]).output().unwrap();
    env.cmd().args(["t", "移行したタスク"]).output().unwrap();
    env.cmd().args(["t", "未完了のタスク"]).output().unwrap();
    env.cmd().args(["sub", "サブ補足項目"]).output().unwrap();

    let mut child = env
        .cmd()
        .arg("review")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"o>s").unwrap();
    }
    child.wait_with_output().unwrap();

    // Set theme = "monokai" in config
    fs::write(env.config_path(), "theme = \"monokai\"\n").unwrap();

    let out = env.cmd().output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Date header: #66d9ef (102, 217, 239) + Bold (\x1b[1;38;2;102;217;239m)
    assert!(
        stdout.contains("\x1b[1;38;2;102;217;239m### 10/21(水)\x1b[0m"),
        "Date header should be #66d9ef Bold in Monokai:\n{:?}",
        stdout
    );
    // Section heading: #fd971f (253, 151, 31) + Bold (\x1b[1;38;2;253;151;31m)
    assert!(
        stdout.contains("\x1b[1;38;2;253;151;31m#### △ やることリスト\x1b[0m"),
        "Section heading should be #fd971f Bold in Monokai:\n{:?}",
        stdout
    );
    // Completed task: #75715e (117, 113, 94) + Dim(2) + Strikethrough(9) (\x1b[2;9;38;2;117;113;94m)
    assert!(
        stdout.contains("\x1b[2;9;38;2;117;113;94mO 完了したタスク\x1b[0m"),
        "Done task should be #75715e Dim+Strikethrough in Monokai:\n{:?}",
        stdout
    );
    // Migrated task: #e6db74 (230, 219, 116) Normal (\x1b[38;2;230;219;116m)
    assert!(
        stdout.contains("\x1b[38;2;230;219;116m> 移行したタスク\x1b[0m"),
        "Migrated task should be #e6db74 Normal in Monokai:\n{:?}",
        stdout
    );
    // Todo task: #f8f8f2 (248, 248, 242) Normal (\x1b[38;2;248;248;242m)
    assert!(
        stdout.contains("\x1b[38;2;248;248;242m• 未完了のタスク\x1b[0m"),
        "Todo task should be #f8f8f2 Normal in Monokai:\n{:?}",
        stdout
    );
    // Subtask: #a6e22e (166, 226, 46) Normal (\x1b[38;2;166;226;46m)
    assert!(
        stdout.contains("\x1b[38;2;166;226;46m  - サブ補足項目\x1b[0m"),
        "Subtask should be #a6e22e Normal in Monokai:\n{:?}",
        stdout
    );
}

/// [TP-19 / BDD 06-2 / EARS-19]
/// Given: 任意のテーマが選択されている / When: 手帳一覧を描画
/// Then: Nerd Fontsや絵文字依存のグリフを含まず標準記号のみで描画される
#[test]
fn tp19_all_themes_render_with_safe_standard_unicode_symbols_only() {
    for theme in ["tokyo-night", "monokai", "nord", "sepia"] {
        let env = TestEnv::new("2026-10-21");
        let out = env.cmd().args(["theme", theme]).output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8_lossy(&out.stdout);

        for ch in stdout.chars() {
            let code = ch as u32;
            // Ensure no Nerd Fonts Private Use Area (U+E000..=U+F8FF, U+F0000..=U+10FFFF)
            // and no emoji/pictograph plane characters (U+1F000..=U+1FFFF)
            assert!(
                !(0xE000..=0xF8FF).contains(&code)
                    && !(0xF0000..=0x10FFFF).contains(&code)
                    && !(0x1F000..=0x1FFFF).contains(&code),
                "Theme '{}' output contained unsafe glyph U+{:04X} ('{}')",
                theme,
                code,
                ch
            );
        }
    }
}

/// [TP-20 / BDD 06-3 / EARS-20]
/// Given: 現在tokyo-nightが適用されている / When: bujo theme nord を実行
/// Then: 設定ファイルが更新され、Nordパレットで即座に手帳が再描画される
#[test]
fn tp20_theme_command_persists_setting_and_immediately_previews_in_new_palette() {
    let env = TestEnv::new("2026-10-21");
    env.cmd().args(["t", "Nordテーマ確認用タスク"]).output().unwrap();

    let out = env.cmd().args(["theme", "nord"]).output().unwrap();
    assert!(out.status.success());

    // Config file is updated
    let cfg_text = fs::read_to_string(env.config_path()).unwrap();
    assert!(cfg_text.contains("theme = \"nord\""), "cfg:\n{}", cfg_text);

    // Output immediately previews the journal in Nord palette (#88c0d0 = 136, 192, 208; #81a1c1 = 129, 161, 193)
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\x1b[1;38;2;136;192;208m### 10/21(水)\x1b[0m"),
        "Should preview date header in Nord #88c0d0:\n{:?}",
        stdout
    );
    assert!(
        stdout.contains("\x1b[1;38;2;129;161;193m#### △ やることリスト\x1b[0m"),
        "Should preview section heading in Nord #81a1c1:\n{:?}",
        stdout
    );
    assert!(stdout.contains("• Nordテーマ確認用タスク"));
}

/// [TP-21 / BDD 06-4 / EARS-21]
/// Given: 不正なテーマ名（例: theme='unknown'）が指定されている / When: bujo を実行
/// Then: エラー終了せず警告を表示した上でデフォルトのtokyo-nightテーマを適用して描画する
#[test]
fn tp21_unknown_theme_name_warns_and_falls_back_to_tokyo_night_without_crashing() {
    let env = TestEnv::new("2026-10-21");
    fs::write(env.config_path(), "theme = \"unknown\"\n").unwrap();

    let out = env.cmd().output().unwrap();
    assert!(
        out.status.success(),
        "bujo should not crash or exit with error when theme is unknown"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    // Warning is displayed
    assert!(
        stdout.contains("unknown") && stdout.contains("tokyo-night"),
        "Should display fallback warning:\n{}",
        stdout
    );
    // Default tokyo-night palette (#7aa2f7 = 122, 162, 247) is applied
    assert!(
        stdout.contains("\x1b[1;38;2;122;162;247m### 10/21(水)\x1b[0m"),
        "Should fall back to tokyo-night #7aa2f7:\n{:?}",
        stdout
    );
}

/// [TP-26 / BDD 08-1 / EARS-26]
/// Given: プラグインが存在しない環境
/// When: yarube reflect で文章を入力し保存
/// Then: '△ ふりかえり'へ文章が永続化され、フック実行のみを静かにスキップして正常終了する
#[test]
fn tp26_bdd08_1_reflect_succeeds_and_silently_skips_when_plugin_missing() {
    let env = TestEnv::new("2026-10-17");
    let non_existent_plugins_dir = env.dir.join("no_such_plugins_dir");

    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[plugins]\ndir = \"{}\"\non_reflect = [\"coach\"]\n",
            non_existent_plugins_dir.display()
        ),
    )
    .unwrap();

    let out = env
        .cmd()
        .args([
            "reflect",
            "プラグイン不在でも確実に保存されるふりかえりテキスト。",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "yarube reflect must exit with code 0 even when plugin does not exist"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.trim().is_empty(),
        "Missing plugin should be silently skipped without error, stderr:\n{}",
        stderr
    );

    let saved = env.read_log("2026-10-17");
    assert!(
        saved.contains("#### △ ふりかえり\nプラグイン不在でも確実に保存されるふりかえりテキスト。\n"),
        "Reflection must be persisted:\n{}",
        saved
    );
}

/// [TP-27 / BDD 08-2 / EARS-27]
/// Given: フックプラグインが異常終了(exit 1)する状態
/// When: yarube reflect を実行
/// Then: ふりかえり保存は成功し（フック起動時点で既にファイルへ保存完了しておりロールバックされない）、本体はクラッシュせず警告のみを出して終了コード0で完了する
#[test]
fn tp27_bdd08_2_reflect_preserves_log_and_exits_zero_when_hook_plugin_fails() {
    let env = TestEnv::new("2026-10-17");
    let plugins_dir = env.dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    // Dummy plugin `yarube-coach` that verifies the log file ALREADY contains the reflection before crashing with exit 1
    let snapshot_file = env.dir.join("log_snapshot_during_hook.md");
    let coach_bin = plugins_dir.join("yarube-coach");
    fs::write(
        &coach_bin,
        format!(
            "#!/bin/sh\ncp \"$YARUBE_LOG_PATH\" \"{}\"\nexit 1\n",
            snapshot_file.display()
        ),
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", coach_bin.to_str().unwrap()])
        .status();

    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[plugins]\ndir = \"{}\"\non_reflect = [\"coach\"]\n",
            plugins_dir.display()
        ),
    )
    .unwrap();

    let out = env
        .cmd()
        .args([
            "reflect",
            "プラグインがクラッシュしても絶対に失われない内省記録。",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "yarube reflect must exit 0 even when hook plugin exits with 1"
    );

    // Verify EARS-26: The file was ALREADY persisted & flushed when the hook started
    let snapshot = fs::read_to_string(&snapshot_file)
        .expect("hook should have been able to read already-flushed log file");
    assert!(
        snapshot.contains("プラグインがクラッシュしても絶対に失われない内省記録。"),
        "Log file must be flushed BEFORE hook executes:\n{}",
        snapshot
    );

    // Verify EARS-27: Saved log is not rolled back after plugin failure
    let saved = env.read_log("2026-10-17");
    assert!(
        saved.contains("#### △ ふりかえり\nプラグインがクラッシュしても絶対に失われない内省記録。\n"),
        "Saved log must remain intact:\n{}",
        saved
    );
}

/// [TP-28 / BDD 08-3 / EARS-28]
/// Given: プラグインが導入済み
/// When: 設定ファイルで plugins.on_reflect = [] または enabled = false を指定
/// Then: 次回実行時からプラグインプロセスが一切起動せずコア機能のみで動作する
#[test]
fn tp28_bdd08_3_selective_plugin_disable_via_config_on_reflect_or_enabled_flag() {
    let env = TestEnv::new("2026-10-17");
    let plugins_dir = env.dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let marker_file = env.dir.join("plugin_ran.txt");
    let coach_bin = plugins_dir.join("yarube-coach");
    fs::write(
        &coach_bin,
        format!("#!/bin/sh\necho \"ran\" >> \"{}\"\n", marker_file.display()),
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", coach_bin.to_str().unwrap()])
        .status();

    // Case 1: on_reflect = []
    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[plugins]\ndir = \"{}\"\non_reflect = []\n",
            plugins_dir.display()
        ),
    )
    .unwrap();

    let out1 = env
        .cmd()
        .args(["reflect", "フック配列が空のときの記録"])
        .output()
        .unwrap();
    assert!(out1.status.success());
    assert!(
        !marker_file.exists(),
        "Plugin should NOT run when on_reflect = []"
    );

    // Case 2: on_reflect = ["coach"] but [plugins.coach] enabled = false
    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[plugins]\ndir = \"{}\"\non_reflect = [\"coach\"]\n\n[plugins.coach]\nenabled = false\n",
            plugins_dir.display()
        ),
    )
    .unwrap();

    let out2 = env
        .cmd()
        .args(["reflect", "enabled=falseのときの記録"])
        .output()
        .unwrap();
    assert!(out2.status.success());
    assert!(
        !marker_file.exists(),
        "Plugin should NOT run when [plugins.coach] enabled = false"
    );
}

/// [TP-29 / BDD 09-1 / EARS-29]
/// Given: プラグインフォルダ（またはPATH）に実行可能な yarube-weekly が配置されている
/// When: yarube weekly --days 7 を実行
/// Then: yarube-weekly に引数 '--days 7' と共通環境変数 YARUBE_* が渡されて実行される
#[test]
fn tp29_bdd09_1_dynamic_subcommand_dispatch_delegates_args_and_env() {
    let env = TestEnv::new("2026-10-17");
    let plugins_dir = env.dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let weekly_bin = plugins_dir.join("yarube-weekly");
    fs::write(
        &weekly_bin,
        "#!/bin/sh\necho \"ARGS:$*\"\necho \"VER:$YARUBE_VERSION\"\necho \"LANG:$YARUBE_LANG\"\necho \"THEME:$YARUBE_THEME_NAME\"\necho \"COLOR:$YARUBE_COLOR_COACH\"\n",
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", weekly_bin.to_str().unwrap()])
        .status();

    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[general]\ntheme = \"nord\"\n\n[plugins]\ndir = \"{}\"\n",
            plugins_dir.display()
        ),
    )
    .unwrap();

    let out = env
        .cmd()
        .args(["weekly", "--days", "7"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ARGS:--days 7"),
        "Subcommand should receive '--days 7', got:\n{}",
        stdout
    );
    assert!(stdout.contains(&format!("VER:{}", env!("CARGO_PKG_VERSION"))));
    assert!(stdout.contains("LANG:ja"));
    assert!(stdout.contains("THEME:nord"));
    assert!(stdout.contains("COLOR:#8fbcbb"));
    assert!(
        !env.log_path("2026-10-17").exists(),
        "Delegating to external subcommand must NOT add 'weekly --days 7' as a todo task"
    );

    // Also verify PATH fallback when binary is not in plugins.dir
    let empty_plugins_dir = env.dir.join("empty_plugins");
    fs::create_dir_all(&empty_plugins_dir).unwrap();
    fs::write(
        env.dir.join("config.toml"),
        format!("[plugins]\ndir = \"{}\"\n", empty_plugins_dir.display()),
    )
    .unwrap();
    let cur_path = env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", plugins_dir.display(), cur_path);
    let out_path = env
        .cmd()
        .env("PATH", new_path)
        .args(["weekly", "--from-path"])
        .output()
        .unwrap();
    assert!(out_path.status.success());
    let stdout_path = String::from_utf8_lossy(&out_path.stdout);
    assert!(stdout_path.contains("ARGS:--from-path"));
}

/// [TP-30 / BDD 09-2 / EARS-30]
/// Given: on_reflect に自作スクリプトが登録されている
/// When: yarube reflect で保存が完了する
/// Then: 子プロセスのstdinに1行JSON（event, date, log_path, added_text）が渡され、環境変数 YARUBE_COLOR_COACH 等が設定される
#[test]
fn tp30_bdd09_2_on_reflect_hook_receives_json_stdin_and_yarube_env_vars() {
    let env = TestEnv::new("2026-10-17");
    let plugins_dir = env.dir.join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let captured_stdin = env.dir.join("captured_stdin.json");
    let captured_env = env.dir.join("captured_env.txt");
    let coach_bin = plugins_dir.join("yarube-coach");
    fs::write(
        &coach_bin,
        format!(
            "#!/bin/sh\ncat > \"{}\"\n{{\n  echo \"VERSION=$YARUBE_VERSION\"\n  echo \"LANG=$YARUBE_LANG\"\n  echo \"LOG_PATH=$YARUBE_LOG_PATH\"\n  echo \"CONFIG_PATH=$YARUBE_CONFIG_PATH\"\n  echo \"THEME_NAME=$YARUBE_THEME_NAME\"\n  echo \"COLOR_COACH=$YARUBE_COLOR_COACH\"\n}} > \"{}\"\necho \"> [Coach] テスト出力です\"\n",
            captured_stdin.display(),
            captured_env.display()
        ),
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", coach_bin.to_str().unwrap()])
        .status();

    // Configure theme = "tokyo-night" and ~ expansion via HOME or direct path
    fs::write(
        env.dir.join("config.toml"),
        format!(
            "[general]\ntheme = \"tokyo-night\"\n\n[plugins]\ndir = \"{}\"\non_reflect = [\"coach\"]\n",
            plugins_dir.display()
        ),
    )
    .unwrap();

    let reflection_body = "テストする気が起きない・・・、仕事やらんでも・・・（未確定情報しかない）。";
    let out = env
        .cmd()
        .args(["reflect", reflection_body])
        .output()
        .unwrap();
    assert!(out.status.success());

    // 1. Verify plugin stdout was inherited to terminal
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("> [Coach] テスト出力です"),
        "Hook stdout should be inherited:\n{}",
        stdout
    );

    // 2. Verify 1-line JSON passed via stdin
    let stdin_json = fs::read_to_string(&captured_stdin).expect("read captured stdin JSON");
    let lines: Vec<&str> = stdin_json.lines().collect();
    assert_eq!(lines.len(), 1, "stdin payload must be single-line JSON");
    let line = lines[0];
    assert!(line.contains("\"event\":\"on_reflect\""), "json: {}", line);
    assert!(line.contains("\"date\":\"2026-10-17\""), "json: {}", line);
    assert!(
        line.contains(&format!("\"added_text\":\"{}\"", reflection_body)),
        "json: {}",
        line
    );
    assert!(line.contains("2026-10-17.md"), "json: {}", line);

    // 3. Verify injected YARUBE_* environment variables
    let env_text = fs::read_to_string(&captured_env).expect("read captured env");
    assert!(env_text.contains(&format!("VERSION={}", env!("CARGO_PKG_VERSION"))));
    assert!(env_text.contains("LANG=ja"));
    assert!(env_text.contains("2026-10-17.md"));
    assert!(env_text.contains("config.toml"));
    assert!(env_text.contains("THEME_NAME=tokyo-night"));
    assert!(env_text.contains("COLOR_COACH=#7dcfff"));
}

/// Verifies tilde (`~/`) expansion, CWD-independence, execution ordering (save -> redraw -> hook),
/// partial `config.toml` default fallback (`on_reflect = ["coach"]`), and `YARUBE_DEBUG=1` diagnostics.
#[test]
fn tp31_tilde_expansion_cwd_independence_and_debug_diagnostics() {
    let env = TestEnv::new("2026-10-17");
    let fake_home = env.dir.join("fake_home");
    let default_plugins_dir = fake_home.join(".local/share/yarube/plugins");
    fs::create_dir_all(&default_plugins_dir).unwrap();

    let coach_bin = default_plugins_dir.join("yarube-coach");
    fs::write(
        &coach_bin,
        "#!/bin/sh\necho \"> [Coach] チルダ展開とCWD非依存のフック出力\"\n",
    )
    .unwrap();
    let _ = Command::new("chmod")
        .args(["+x", coach_bin.to_str().unwrap()])
        .status();

    // Existing config.toml that has NO `[plugins]` or `on_reflect` specified (must default to ~/.local/share/yarube/plugins and ["coach"])
    fs::write(env.dir.join("config.toml"), "theme = \"nord\"\n").unwrap();

    // Run from a completely different working directory (`/tmp`) with `HOME` set to `fake_home` and `YARUBE_DEBUG=1`
    let out = env
        .cmd()
        .current_dir(env::temp_dir())
        .env("HOME", &fake_home)
        .env("YARUBE_DEBUG", "1")
        .args(["reflect", "どこから実行しても発火するテスト"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // 1. Verify debug diagnostic output on stderr
    assert!(
        stderr.contains("[debug] checking plugin:") && stderr.contains("yarube-coach (exists: true"),
        "YARUBE_DEBUG=1 should output plugin check path and existence on stderr:\n{}",
        stderr
    );

    // 2. Verify execution order: journal redraw (`#### △ ふりかえり`) appears BEFORE hook output (`> [Coach] ...`)
    let redraw_pos = stdout
        .find("どこから実行しても発火するテスト")
        .expect("journal redraw must appear in stdout");
    let coach_pos = stdout
        .find("> [Coach] チルダ展開とCWD非依存のフック出力")
        .expect("hook output must appear in stdout");
    assert!(
        redraw_pos < coach_pos,
        "Journal redraw must happen BEFORE on_reflect hook output:\n{}",
        stdout
    );
}



