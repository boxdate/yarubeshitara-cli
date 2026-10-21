use crate::domain::date::LocalDate;
use crate::domain::journal::{DailyLog, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMatch {
    pub task: Task,
    pub task_matched: bool,
    pub matched_subtask_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaySearchResult {
    pub date: LocalDate,
    pub header: String,
    pub matched_tasks: Vec<TaskMatch>,
    pub matched_reflection_lines: Vec<String>,
}

/// Searches across a list of `DailyLog` entries for `query` (case-insensitive substring match).
/// Returns all days that have at least one match in either tasks/subtasks or reflection (TP-15 / EARS-15).
pub fn search_logs(logs: &[DailyLog], query: &str) -> Vec<DaySearchResult> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Vec::new();
    }
    let query_lower = trimmed_query.to_lowercase();
    let mut results = Vec::new();

    for log in logs {
        let mut matched_tasks = Vec::new();
        for task in &log.tasks {
            let task_matched = contains_ignore_case(&task.text, &query_lower);
            let mut matched_subtask_indices = Vec::new();
            for (s_idx, sub) in task.subtasks.iter().enumerate() {
                if contains_ignore_case(sub, &query_lower) {
                    matched_subtask_indices.push(s_idx);
                }
            }
            if task_matched || !matched_subtask_indices.is_empty() {
                matched_tasks.push(TaskMatch {
                    task: task.clone(),
                    task_matched,
                    matched_subtask_indices,
                });
            }
        }

        let mut matched_reflection_lines = Vec::new();
        for line in &log.reflection {
            if contains_ignore_case(line, &query_lower) {
                matched_reflection_lines.push(line.clone());
            }
        }

        if !matched_tasks.is_empty() || !matched_reflection_lines.is_empty() {
            results.push(DaySearchResult {
                date: log.date,
                header: log.header.clone(),
                matched_tasks,
                matched_reflection_lines,
            });
        }
    }

    results
}

fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    haystack.to_lowercase().contains(needle_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_logs_matches_tasks_subtasks_and_reflections() {
        let d1 = LocalDate::new(2026, 10, 17);
        let mut log1 = DailyLog::new(d1);
        log1.add_task("PodmanにGitbucketを立ち上げる");
        log1.add_subtask("Lubuntu -> Xubuntuが良いか").unwrap();
        log1.append_reflection("EndeavourOSにしたあき！\nUXって大事。");

        let d2 = LocalDate::new(2026, 10, 18);
        let mut log2 = DailyLog::new(d2);
        log2.add_task("別の日タスク");

        let logs = vec![log1, log2];

        let res = search_logs(&logs, "EndeavourOS");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].date, d1);
        assert_eq!(res[0].matched_reflection_lines, vec!["EndeavourOSにしたあき！"]);

        let res_sub = search_logs(&logs, "Xubuntu");
        assert_eq!(res_sub.len(), 1);
        assert_eq!(res_sub[0].matched_tasks.len(), 1);
        assert_eq!(res_sub[0].matched_tasks[0].matched_subtask_indices, vec![0]);
    }
}
