use crate::domain::date::LocalDate;
use crate::domain::i18n::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// 未完了 (`•`)
    Todo,
    /// 完了 (`O`)
    Done,
    /// 翌日へ移行済み (`>`)
    Migrated,
    /// 取消 (`~`)
    Cancelled,
}

impl TaskStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "•",
            TaskStatus::Done => "O",
            TaskStatus::Migrated => ">",
            TaskStatus::Cancelled => "~",
        }
    }

    pub fn from_symbol(sym: &str) -> Option<Self> {
        match sym {
            "•" | "*" => Some(TaskStatus::Todo),
            "O" | "o" | "x" | "X" => Some(TaskStatus::Done),
            ">" => Some(TaskStatus::Migrated),
            "~" => Some(TaskStatus::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub status: TaskStatus,
    pub text: String,
    pub subtasks: Vec<String>,
}

impl Task {
    pub fn new_todo(text: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Todo,
            text: text.into(),
            subtasks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightTarget {
    None,
    Task(usize),
    Subtask { task_index: usize, sub_index: usize },
    Reflection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyLog {
    pub date: LocalDate,
    pub header: String,
    pub tasks: Vec<Task>,
    pub reflection: Vec<String>,
}

impl DailyLog {
    pub fn new(date: LocalDate) -> Self {
        Self::new_with_lang(date, Language::Ja)
    }

    pub fn new_with_lang(date: LocalDate, lang: Language) -> Self {
        Self {
            date,
            header: date.format_header_for(lang),
            tasks: Vec::new(),
            reflection: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.reflection.iter().all(|l| l.trim().is_empty())
    }

    pub fn add_task(&mut self, text: impl Into<String>) -> usize {
        self.tasks.push(Task::new_todo(text));
        self.tasks.len() - 1
    }

    pub fn add_subtask(&mut self, text: impl Into<String>) -> Result<(usize, usize), &'static str> {
        let last_idx = self
            .tasks
            .len()
            .checked_sub(1)
            .ok_or("サブ項目を追加できる親タスクがまだありません。先に `yarube <タスク内容>` でタスクを追加してください。")?;
        self.tasks[last_idx].subtasks.push(text.into());
        let sub_idx = self.tasks[last_idx].subtasks.len() - 1;
        Ok((last_idx, sub_idx))
    }

    pub fn append_reflection(&mut self, content: &str) {
        let trimmed = content.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.trim().is_empty() {
            return;
        }
        for line in trimmed.lines() {
            self.reflection.push(line.to_string());
        }
    }

    pub fn to_markdown(&self) -> String {
        self.to_markdown_with_lang(Language::Ja)
    }

    pub fn to_markdown_with_lang(&self, lang: Language) -> String {
        let mut out = String::new();
        out.push_str(&self.date.format_header_for(lang));
        out.push_str("\n\n");
        out.push_str(lang.todo_section_header());
        out.push('\n');
        for task in &self.tasks {
            out.push_str(&format!("{} {}\n", task.status.symbol(), task.text));
            for sub in &task.subtasks {
                out.push_str(&format!("  - {}\n", sub));
            }
        }
        out.push('\n');
        out.push_str(lang.reflection_section_header());
        out.push('\n');
        for line in &self.reflection {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// Parses Markdown content regardless of whether section headings are written in Japanese
    /// (`#### △ やることリスト` / `#### △ ふりかえり`) or English (`#### △ Todo List` / `#### △ Reflection`) (TP-17 / EARS-17).
    pub fn parse_markdown(date: LocalDate, content: &str) -> Self {
        let mut log = Self::new(date);
        let mut section = Section::Header;

        for raw_line in content.lines() {
            let line = raw_line.trim_end_matches('\r');
            let trimmed = line.trim();

            if trimmed.starts_with("### ") && !trimmed.starts_with("#### ") {
                log.header = trimmed.to_string();
                continue;
            }
            if Language::is_todo_heading(trimmed) {
                section = Section::Todo;
                continue;
            }
            if Language::is_reflection_heading(trimmed) {
                section = Section::Reflection;
                continue;
            }

            match section {
                Section::Header => {}
                Section::Todo => {
                    if trimmed.is_empty() {
                        continue;
                    }
                    if (line.starts_with("  ") || line.starts_with('\t'))
                        && (trimmed.starts_with("- ")
                            || trimmed.starts_with("L ")
                            || trimmed.starts_with("└ "))
                    {
                        let sub_text = if let Some(rest) = trimmed.strip_prefix("- ") {
                            rest
                        } else if let Some(rest) = trimmed.strip_prefix("L ") {
                            rest
                        } else if let Some(rest) = trimmed.strip_prefix("└ ") {
                            rest
                        } else {
                            trimmed
                        };
                        if let Some(last_task) = log.tasks.last_mut() {
                            last_task.subtasks.push(sub_text.to_string());
                        }
                        continue;
                    }

                    if let Some((sym, rest)) = trimmed.split_once(' ') {
                        if let Some(status) = TaskStatus::from_symbol(sym) {
                            log.tasks.push(Task {
                                status,
                                text: rest.to_string(),
                                subtasks: Vec::new(),
                            });
                            continue;
                        }
                    }
                }
                Section::Reflection => {
                    log.reflection.push(line.to_string());
                }
            }
        }

        while log.reflection.last().is_some_and(|l| l.trim().is_empty()) {
            log.reflection.pop();
        }

        log
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Header,
    Todo,
    Reflection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_roundtrip() {
        let date = LocalDate::new(2026, 10, 17);
        let mut log = DailyLog::new(date);
        log.add_task("PodmanにGitbucketを立ち上げる");
        log.add_subtask("Lubuntu -> Xubuntuが良いか").unwrap();
        log.tasks[0].status = TaskStatus::Migrated;
        log.append_reflection("EndeavourOSにしたあき！\nUXって大事。");

        let md = log.to_markdown();
        let parsed = DailyLog::parse_markdown(date, &md);
        assert_eq!(parsed, log);
    }

    #[test]
    fn test_bilingual_markdown_parsing() {
        let date = LocalDate::new(2026, 10, 21);
        let en_markdown = "\
### 10/21(Wed)

#### △ Todo List
• Write bilingual parser tests
  - Verify English and Japanese headings

#### △ Reflection
Bilingual support works smoothly!
";
        let parsed = DailyLog::parse_markdown(date, en_markdown);
        assert_eq!(parsed.header, "### 10/21(Wed)");
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.tasks[0].text, "Write bilingual parser tests");
        assert_eq!(
            parsed.tasks[0].subtasks,
            vec!["Verify English and Japanese headings"]
        );
        assert_eq!(parsed.reflection, vec!["Bilingual support works smoothly!"]);
    }
}
