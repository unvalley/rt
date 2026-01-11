mod detect;
mod exec;
mod parser;
mod tasks;

use bpaf::Bpaf;
use std::path::PathBuf;

fn main() {
    let cli = parse_cli();
    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            classify_error(&err)
        }
    };

    std::process::exit(exit_code);
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
struct Args {
    /// Task name to run in your task runner files (e.g. `build`, `test`).
    #[bpaf(positional("task"))]
    task: Option<String>,
    #[bpaf(positional("args"), many)]
    rest: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Cli {
    pub task: Option<String>,
    pub passthrough: Vec<String>,
}

pub fn parse_cli() -> Cli {
    let raw = args().run();
    let passthrough = match raw.rest.split_first() {
        Some((first, rest)) if first == "--" => rest.to_vec(),
        Some((_first, _rest)) => raw.rest,
        None => Vec::new(),
    };

    Cli {
        task: raw.task,
        passthrough,
    }
}

/// Runs tasks based on the provided CLI arguments.
fn run(cli: Cli) -> Result<i32, RtError> {
    let cwd = std::env::current_dir().map_err(RtError::Io)?;
    let detection = detect::detect_runner(&cwd)?;

    let task = match cli.task {
        Some(task) => Some(task),
        None => tasks::select_task(detection.runner)?,
    };

    match task {
        Some(task) => exec::run(detection.runner, &task, &cli.passthrough),
        None => Ok(0),
    }
}

fn classify_error(err: &RtError) -> i32 {
    match err {
        RtError::NoRunnerFound { .. }
        | RtError::ToolMissing { .. }
        | RtError::NoTasks { .. }
        | RtError::ListFailed { .. } => 3,
        RtError::Prompt(_) | RtError::Io(_) | RtError::Spawn(_) => 2,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RtError {
    #[error("no runner found in {cwd:?}")]
    NoRunnerFound { cwd: PathBuf },
    #[error("required tool not found in PATH: {tool}")]
    ToolMissing { tool: &'static str },
    #[error("no tasks found using {tool}")]
    NoTasks { tool: &'static str },
    #[error("failed to list tasks using {tool} (exit code {status})")]
    ListFailed { tool: &'static str, status: i32 },
    #[error("prompt error: {0}")]
    Prompt(#[from] inquire::error::InquireError),
    #[error("io error: {0}")]
    Io(std::io::Error),
    #[error("failed to spawn command: {0}")]
    Spawn(std::io::Error),
}
