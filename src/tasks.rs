use std::collections::BTreeSet;
use std::fmt;
use std::process::Command;

use inquire::error::InquireError;

use crate::detect::{Runner, runner_command};
use crate::error::RtError;

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

pub fn select_task(runner: Runner) -> Result<Option<String>, RtError> {
    let tasks = list_tasks(runner)?;
    if tasks.is_empty() {
        return Err(RtError::NoTasks {
            tool: runner_command(runner),
        });
    }

    match inquire::Select::new("Select task", tasks).prompt() {
        Ok(item) => Ok(Some(item.name)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(err) => Err(RtError::Prompt(err)),
    }
}

fn list_tasks(runner: Runner) -> Result<Vec<TaskItem>, RtError> {
    let cwd = std::env::current_dir().map_err(RtError::Io)?;
    let program = runner_command(runner);
    ensure_tool(program)?;

    let mut last_status = 2;
    for args in list_command_variants(runner) {
        let output = Command::new(program)
            .args(args)
            .current_dir(&cwd)
            .output()
            .map_err(RtError::Spawn)?;

        let status = output.status.code().unwrap_or(2);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if status == 0 {
            return Ok(parse_tasks(runner, &stdout));
        }

        if runner == Runner::Make && !stdout.trim().is_empty() {
            return Ok(parse_tasks(runner, &stdout));
        }

        last_status = status;
    }

    Err(RtError::ListFailed {
        tool: program,
        status: last_status,
    })
}

fn list_command_variants(runner: Runner) -> Vec<Vec<&'static str>> {
    match runner {
        Runner::Just => vec![vec!["--list", "--unsorted"]],
        Runner::Taskfile => vec![vec!["--list-all"]],
        Runner::CargoMake => vec![
            vec!["make", "--list-all-steps"],
            vec!["make", "--list-all"],
            vec!["make", "--list"],
        ],
        Runner::Make => vec![vec!["-rR", "-qp"], vec!["-qp"]],
    }
}

fn parse_tasks(runner: Runner, output: &str) -> Vec<TaskItem> {
    match runner {
        Runner::Just => parse_just(output),
        Runner::Taskfile => parse_taskfile(output),
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

fn parse_taskfile(output: &str) -> Vec<TaskItem> {
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
    let has_files_section = output.contains("\n# Files");
    let mut in_files = !has_files_section;
    let mut names = BTreeSet::new();

    for line in output.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("# Files") {
            in_files = true;
            continue;
        }
        if trimmed.starts_with("# Finished") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix(".PHONY:") {
            for name in rest.split_whitespace() {
                names.insert(name.to_string());
            }
            continue;
        }
        if !in_files || trimmed.starts_with('#') || line.starts_with('\t') || line.starts_with(' ')
        {
            continue;
        }

        let (target, _) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let name = target.trim();
        if !is_make_target_name(name) {
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

fn is_make_target_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('%')
        && !name.contains('$')
        && !name.contains('=')
        && name != "Makefile"
        && name != "makefile"
        && name != "GNUmakefile"
}

fn ensure_tool(tool: &'static str) -> Result<(), RtError> {
    match which::which(tool) {
        Ok(_) => Ok(()),
        Err(_) => Err(RtError::ToolMissing { tool }),
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
        let tasks = parse_taskfile(output);
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
