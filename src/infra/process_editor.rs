use crate::domain::date::LocalDate;
use crate::ports::EditorLauncher;
use std::env;
use std::fs;
use std::io;
use std::process::Command;

pub struct ProcessEditorLauncher;

impl EditorLauncher for ProcessEditorLauncher {
    fn edit_reflection(&self, editor_cmd: &str, date: LocalDate, header: &str) -> io::Result<String> {
        let tmp_dir = env::temp_dir();
        let tmp_path = tmp_dir.join(format!(
            "yarube-reflect-{}-{}.md",
            date.format_ymd(),
            std::process::id()
        ));

        let template = format!(
            "\n# --- {} の『△ ふりかえり』を入力して保存・終了してください ---\n# (使用エディタ: {} / '#' で始まる行は保存時に除外されます)\n",
            header.trim_start_matches('#').trim(),
            editor_cmd
        );
        fs::write(&tmp_path, &template)?;

        let status = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(format!("{} \"{}\"", editor_cmd, tmp_path.display()))
                .status()?
        } else {
            Command::new("sh")
                .arg("-c")
                .arg(format!("{} \"$1\"", editor_cmd))
                .arg("sh")
                .arg(&tmp_path)
                .status()?
        };

        if !status.success() {
            let _ = fs::remove_file(&tmp_path);
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "エディタ (`{}`) が異常終了したため、ふりかえりの保存を中止しました。",
                    editor_cmd
                ),
            ));
        }

        let raw_content = fs::read_to_string(&tmp_path).unwrap_or_default();
        let _ = fs::remove_file(&tmp_path);

        let filtered: Vec<&str> = raw_content
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .collect();

        Ok(filtered.join("\n").trim().to_string())
    }
}
