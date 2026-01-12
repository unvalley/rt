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
        .map(|task| task.name.chars().count())
        .max()
        .unwrap_or(0);

    let items: Vec<TaskChoice> = tasks
        .into_iter()
        .map(|task| TaskChoice::new(task, max_name_len))
        .collect();

    match inquire::Select::new("Select task", items).prompt() {
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
        Runner::CargoMake => vec![
            vec!["make", "--list-all-steps"],
            vec!["make", "--list-all"],
            vec!["make", "--list"],
        ],
        Runner::Makefile => vec![vec!["-rR", "-qp"], vec!["-qp"]],
    }
}
