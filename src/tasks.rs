use inquire::error::InquireError;
use std::fmt;

use crate::RtError;
use crate::detect::{Runner, runner_command};
use crate::exec::base_command;
use crate::parser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    pub name: String,
    pub description: Option<String>,
}

impl fmt::Display for TaskItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.description {
            Some(desc) => write!(f, "{}  -  {}", self.name, desc),
            None => write!(f, "{}", self.name),
        }
    }
}

/// Prompts the user to select a task from the given runner's task list.
pub fn select_task(runner: Runner) -> Result<Option<String>, RtError> {
    let tasks = list_tasks(runner)?;
    if tasks.is_empty() {
        return Err(RtError::NoTasks {
            tool: runner_command(runner),
        });
    }

    let max_name_len = tasks
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(0);

    let items: Vec<TaskChoice> = tasks
        .into_iter()
        .map(|t| TaskChoice::new(t, max_name_len))
        .collect();

    let items_len = items.len();

    match inquire::Select::new("Select task", items)
        .with_page_size(10)
        .with_scorer(&move |input, option, string_value, idx| {
            let _ = string_value;
            score_task(input, &option.name, idx, items_len)
        })
        .prompt()
    {
        Ok(item) => Ok(Some(item.name)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(err) => Err(RtError::Prompt(err)),
    }
}
#[derive(Debug, Clone)]
struct TaskChoice {
    name: String,
    display: String,
}

impl TaskChoice {
    fn new(task: TaskItem, width: usize) -> Self {
        let display = match task.description {
            Some(desc) => format!("{:width$}  -  {}", task.name, desc, width = width),
            None => task.name.clone(),
        };
        Self {
            name: task.name,
            display,
        }
    }
}

impl fmt::Display for TaskChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display)
    }
}

fn score_task(
    input: &str,
    task_name: &str,
    idx: usize,
    items_len: usize,
) -> Option<i64> {
    let input = input.trim();
    if input.is_empty() {
        return Some(items_len.saturating_sub(idx) as i64);
    }

    let input_lower = input.to_ascii_lowercase();
    let name_lower = task_name.to_ascii_lowercase();
    let exact = name_lower == input_lower;
    let prefix = !exact && name_lower.starts_with(&input_lower);
    let contains = !exact && !prefix && name_lower.contains(&input_lower);

    let boost = if exact {
        10_000_000_i64
    } else if prefix {
        5_000_000_i64
    } else if contains {
        1_000_000_i64
    } else {
        return None;
    };
    Some(boost.saturating_add(items_len.saturating_sub(idx) as i64))
}

/// Lists tasks for the given runner by invoking its list command.
fn list_tasks(runner: Runner) -> Result<Vec<TaskItem>, RtError> {
    let mut last_status = 2;
    for args in list_command_variants(runner) {
        let current_dir = std::env::current_dir().map_err(RtError::Io)?;
        let mut command = base_command(runner)?;
        let output = command
            .args(args)
            .current_dir(&current_dir)
            .output()
            .map_err(RtError::Spawn)?;

        let status = output.status.code().unwrap_or(2);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if status == 0 {
            return Ok(parser::parse_tasks(runner, &stdout));
        }

        if runner == Runner::Makefile && !stdout.trim().is_empty() {
            return Ok(parser::parse_tasks(runner, &stdout));
        }

        last_status = status;
    }

    Err(RtError::ListFailed {
        tool: runner_command(runner),
        status: last_status,
    })
}

/// Returns possible command variants to list tasks for the given runner.
fn list_command_variants(runner: Runner) -> Vec<Vec<&'static str>> {
    match runner {
        Runner::Justfile => vec![vec!["--list", "--unsorted"]],
        Runner::Taskfile => vec![vec!["--list-all"]],
        Runner::Maskfile => vec![vec!["--introspect"]],
        Runner::Mise => vec![vec!["tasks", "ls", "--json"]],
        Runner::CargoMake => vec![
            vec!["make", "--list-all-steps"],
            vec!["make", "--list-all"],
            vec!["make", "--list"],
        ],
        Runner::Makefile => vec![vec!["-rR", "-qp"], vec!["-qp"]],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_task_prefers_exact_over_prefix() {
        let items_len = 2;
        let exact = score_task("format", "format", 1, items_len).unwrap();
        let prefix = score_task("format", "format-rust", 0, items_len).unwrap();

        assert!(exact > prefix);
    }

    #[test]
    fn score_task_filters_non_matches() {
        let items_len = 1;
        let score = score_task("fmt", "build", 0, items_len);
        assert!(score.is_none());
    }

    #[test]
    fn score_task_keeps_stable_order_for_equal_scores() {
        let items_len = 3;
        let first = score_task("foo", "foobar", 0, items_len).unwrap();
        let second = score_task("foo", "foobaz", 1, items_len).unwrap();

        assert!(first > second);
    }

    #[test]
    fn score_task_does_not_include_fuzzy_only_matches() {
        let items_len = 2;
        let exact = score_task("test-unit", "test-unit", 0, items_len);
        let fuzzy = score_task("test-unit", "test-integration", 1, items_len);

        assert!(exact.is_some());
        assert!(fuzzy.is_none());
    }
}
