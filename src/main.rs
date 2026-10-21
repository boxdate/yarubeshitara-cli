pub mod cli;
pub mod domain;
pub mod infra;
pub mod ports;
pub mod ui;

use crate::cli::{parse_args, CliCommand};
use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;
use crate::domain::journal::HighlightTarget;
use crate::domain::search::search_logs;
use crate::infra::fs_storage::FsStorage;
use crate::infra::process_editor::ProcessEditorLauncher;
use crate::ports::{ConfigRepository, JournalRepository};
use crate::ui::browse::{run_browse, run_navigate_relative};
use crate::ui::config_ui::{run_config_editor, run_config_lang, run_config_theme};
use crate::ui::reflect::run_reflect;
use crate::ui::renderer::{render_daily_log_with_options, render_search_results};
use crate::ui::review::run_review;
use std::env;
use std::process;

fn print_help(lang: Language) {
    match lang {
        Language::Ja => {
            println!("Yarubeshitara! (yarube) - やるべしたら！ CLIデイリー手帳 (『△ やることリスト』＋『△ ふりかえり』)");
            println!();
            println!("使い方:");
            println!("  yarube                             当日の手帳を表示（未作成時はテンプレートを自動生成）");
            println!("  yarube t <タスク内容...>           『△ やることリスト』末尾へタスク(•)を追記");
            println!("  yarube <タスク内容...>             記号省略時もデフォルトで『△ やることリスト』へ追記");
            println!("  yarube sub <補足内容...>           直前タスクの配下にサブ項目(  - )をインデント追記");
            println!("  yarube review                      未完了タスク(•)を1問1答で棚卸し(O:完了 / >:翌日移行 / ~:取消)");
            println!("  yarube reflect [テキスト...]       エディタ起動（または直接テキスト指定）で『△ ふりかえり』へ追記");
            println!("  yarube theme [テーマ名]            カラーテーマ切替 (tokyo-night / monokai / nord / sepia)");
            println!("  yarube config editor [コマンド...] ふりかえり用エディタを対話選択または直接設定");
            println!("  yarube config lang [ja|en]         表示言語（日本語 / 英語）を切り替え");
            println!("  yarube prev / yarube -1 / -N       直前の記録日（N記録日前）の手帳を整形表示");
            println!("  yarube next / yarube +1 / +N       直後の記録日（N記録日後）の手帳を整形表示");
            println!("  yarube browse                      記録済みの手帳を前後にパラパラめくる対話ブラウズモード");
            println!("  yarube search <キーワード...>      全デイリーログからキーワード一致する日付・ブロックを抽出表示");
        }
        Language::En => {
            println!("Yarubeshitara! (yarube) - CLI Daily Todo & Reflection ('△ Todo List' + '△ Reflection')");
            println!();
            println!("Usage:");
            println!("  yarube                             Show today's journal (auto-generates template if new)");
            println!("  yarube t <task text...>            Append task (•) to '△ Todo List'");
            println!("  yarube <task text...>              Append task (•) by default even without symbol");
            println!("  yarube sub <note...>               Append indented subtask (  - ) under the preceding task");
            println!("  yarube review                      Interactively review pending tasks (O:Done / >:Migrate / ~:Cancel)");
            println!("  yarube reflect [text...]           Open editor (or append inline text) to '△ Reflection'");
            println!("  yarube theme [name]                Switch color theme (tokyo-night / monokai / nord / sepia)");
            println!("  yarube config editor [cmd...]      Configure reflection editor interactively or directly");
            println!("  yarube config lang [ja|en]         Configure display language (Japanese / English)");
            println!("  yarube prev / yarube -1 / -N       Show previous recorded journal (N recorded days back)");
            println!("  yarube next / yarube +1 / +N       Show next recorded journal (N recorded days forward)");
            println!("  yarube browse                      Interactive page-turning mode across recorded journals");
            println!("  yarube search <keyword...>         Search all journals and highlight matching blocks");
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error / エラー: {}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = parse_args(&args)?;
    let storage = FsStorage::from_env()?;
    let cfg = storage.load_config().unwrap_or_default();
    let lang = cfg.resolve_language();
    let theme_res = cfg.resolve_theme();
    let theme = theme_res.theme;

    if command == CliCommand::Help {
        print_help(lang);
        return Ok(());
    }

    // If a configured theme name is unknown (and we're not currently executing `bujo theme` which prints its own warning),
    // output a non-fatal warning and fall back to default `tokyo-night` (TP-21 / EARS-21).
    if !matches!(command, CliCommand::Theme(_)) {
        if let Some(ref unknown) = theme_res.unknown_requested {
            println!("{}", lang.theme_unknown_warning(unknown));
        }
    }

    let editor_launcher = ProcessEditorLauncher;
    let today = LocalDate::today();

    match command {
        CliCommand::Help => unreachable!(),
        CliCommand::ShowToday => {
            let res = storage.load_or_init(today)?;
            render_daily_log_with_options(
                &res.log,
                HighlightTarget::None,
                res.newly_created,
                lang,
                theme,
            );
        }
        CliCommand::AddTask(text) => {
            let mut res = storage.load_or_init(today)?;
            let idx = res.log.add_task(text);
            storage.save(&res.log)?;
            render_daily_log_with_options(&res.log, HighlightTarget::Task(idx), false, lang, theme);
        }
        CliCommand::AddSubtask(text) => {
            let mut res = storage.load_or_init(today)?;
            let (task_index, sub_index) = res.log.add_subtask(text)?;
            storage.save(&res.log)?;
            render_daily_log_with_options(
                &res.log,
                HighlightTarget::Subtask {
                    task_index,
                    sub_index,
                },
                false,
                lang,
                theme,
            );
        }
        CliCommand::Review => {
            let res = storage.load_or_init(today)?;
            run_review(&storage, &storage, &editor_launcher, res.log)?;
        }
        CliCommand::Reflect(inline) => {
            let res = storage.load_or_init(today)?;
            run_reflect(&storage, &storage, &editor_launcher, res.log, inline)?;
        }
        CliCommand::ConfigEditor(direct_editor) => {
            run_config_editor(&storage, direct_editor)?;
        }
        CliCommand::ConfigLang(direct_lang) => {
            run_config_lang(&storage, direct_lang)?;
        }
        CliCommand::Theme(direct_theme) => {
            run_config_theme(&storage, &storage, today, direct_theme)?;
        }
        CliCommand::NavigateRelative(offset) => {
            run_navigate_relative(&storage, today, offset, lang, theme)?;
        }
        CliCommand::Browse => {
            run_browse(&storage, today, lang, theme)?;
        }
        CliCommand::Search(query) => {
            let logs = storage.load_all_logs()?;
            let results = search_logs(&logs, &query);
            render_search_results(&query, &results, lang, theme);
        }
    }

    Ok(())
}
