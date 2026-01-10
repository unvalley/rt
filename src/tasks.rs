use std::collections::BTreeSet;
use std::fmt;
use std::process::Command;

use inquire::error::InquireError;

use crate::detect::{runner_command, Runner};
use crate::error::RiError;

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

pub fn select_task(runner: Runner) -> Result<Option<String>, RiError> {
    let tasks = list_tasks(runner)?;
    if tasks.is_empty() {
        return Err(RiError::NoTasks {
            tool: runner_command(runner),
        });
    }

    match inquire::Select::new("Select task", tasks).prompt() {
        Ok(item) => Ok(Some(item.name)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(err) => Err(RiError::Prompt(err)),
    }
}

fn list_tasks(runner: Runner) -> Result<Vec<TaskItem>, RiError> {
    let cwd = std::env::current_dir().map_err(RiError::Io)?;
    let program = runner_command(runner);
    ensure_tool(program)?;

    let mut last_status = 2;
    for args in list_command_variants(runner) {
        let output = Command::new(program)
            .args(args)
            .current_dir(&cwd)
            .output()
            .map_err(RiError::Spawn)?;

        match output.status.code() {
            Some(0) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return Ok(parse_tasks(runner, &stdout));
            }
            Some(code) => last_status = code,
            None => last_status = 2,
        }
    }

    Err(RiError::ListFailed {
        tool: program,
        status: last_status,
    })
}

fn list_command_variants(runner: Runner) -> Vec<Vec<&'static str>> {
    match runner {
        Runner::Just => vec![vec!["--list", "--unsorted"]],
        Runner::Task => vec![vec!["--list-all"]],
        Runner::CargoMake => vec![
            vec!["make", "--list-all-steps"],
            vec!["make", "--list-all"],
            vec!["make", "--list"],
        ],
        Runner::Make => vec![vec!["-qp"]],
    }
}

fn parse_tasks(runner: Runner, output: &str) -> Vec<TaskItem> {
    match runner {
        Runner::Just => parse_just(output),
        Runner::Task => parse_task(output),
        Runner::CargoMake => parse_cargo_make(output),
        Runner::Make => parse_make(output),
    }
}

fn parse_just(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Available") || line.starts_with("Recipes") {
            continue;
        }

        let (left, desc) = match line.split_once('#') {
            Some((left, desc)) => (left.trim(), Some(desc.trim())),
            None => (line, None),
        };

        let name = left.split_whitespace().next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }

        let description = desc.filter(|d| !d.is_empty()).map(|d| d.to_string());
        items.push(TaskItem {
            name: name.to_string(),
            description,
        });
    }
    items
}

fn parse_task(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let mut line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("task:") || line.starts_with("Available") {
            continue;
        }

        if let Some(stripped) = line.strip_prefix("* ") {
            line = stripped;
        } else if let Some(stripped) = line.strip_prefix("- ") {
            line = stripped;
        }

        let (name, desc) = match line.split_once(':') {
            Some((name, desc)) => (name.trim(), Some(desc.trim())),
            None => (line.trim(), None),
        };

        if name.is_empty() {
            continue;
        }

        let description = desc.filter(|d| !d.is_empty()).map(|d| d.to_string());
        items.push(TaskItem {
            name: name.to_string(),
            description,
        });
    }
    items
}

fn parse_cargo_make(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.ends_with(':') || line.starts_with("Available") || line.starts_with("Tasks") {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }

        let description = parts
            .next()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(|d| d.to_string());

        items.push(TaskItem {
            name: name.to_string(),
            description,
        });
    }
    items
}

fn parse_make(output: &str) -> Vec<TaskItem> {
    let mut names = BTreeSet::new();
    for line in output.lines() {
        if line.is_empty() || line.starts_with('#') || line.starts_with('\t') || line.starts_with(' ') {
            continue;
        }

        let (target, _) = match line.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let name = target.trim();
        if name.is_empty()
            || name.starts_with('.')
            || name.contains('%')
            || name.contains('$')
            || name.contains('=')
        {
            continue;
        }

        names.insert(name.to_string());
    }

    names
        .into_iter()
        .map(|name| TaskItem {
            name,
            description: None,
        })
        .collect()
}

fn ensure_tool(tool: &'static str) -> Result<(), RiError> {
    match which::which(tool) {
        Ok(_) => Ok(()),
        Err(_) => Err(RiError::ToolMissing { tool }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_just_list() {
        let output = "\
Available recipes:
    build  # build project
    test
";
        let tasks = parse_just(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("build project"));
        assert_eq!(tasks[1].name, "test");
        assert_eq!(tasks[1].description, None);
    }

    #[test]
    fn parse_task_list() {
        let output = "\
task: Available tasks for this project:
* build: Build the project
* test: Run tests
";
        let tasks = parse_task(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("Build the project"));
    }

    #[test]
    fn parse_cargo_make_list() {
        let output = "\
Tasks:
build        Build the project
test         Run tests
";
        let tasks = parse_cargo_make(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("Build the project"));
    }

    #[test]
    fn parse_make_list() {
        let output = "\
all: deps build
.PHONY: all
install:
\t@echo install
%.o: %.c
";
        let tasks = parse_make(output);
        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["all", "install"]);
    }
}
