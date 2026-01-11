mod cli;
mod detect;
mod exec;
mod parser;
mod tasks;

use std::path::PathBuf;

fn main() {
    let cli = cli::parse();
    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            classify_error(&err)
        }
    };

    std::process::exit(exit_code);
}

/// Runs tasks based on the provided CLI arguments.
fn run(cli: cli::Cli) -> Result<i32, RtError> {
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
