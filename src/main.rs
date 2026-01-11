mod detect;
mod exec;
mod parser;
mod tasks;

use bpaf::Bpaf;
use inquire::error::InquireError;
use std::fmt;
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

    if let Some(task) = cli.task {
        let detection = detect::detect_runner(&cwd)?;
        return exec::run(detection.runner, &task, &cli.passthrough);
    }

    let detections = detect::detect_runners(&cwd)?;
    let runner = if detections.len() == 1 {
        Some(detections[0].runner)
    } else {
        select_runner(detections)?
    };

    let runner = match runner {
        Some(runner) => runner,
        None => return Ok(0),
    };

    let task = tasks::select_task(runner)?;
    match task {
        Some(task) => exec::run(runner, &task, &cli.passthrough),
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

struct RunnerItem {
    runner: detect::Runner,
    runner_file: PathBuf,
}

impl fmt::Display for RunnerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filename = self
            .runner_file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.runner_file.to_string_lossy().into_owned());
        write!(f, "{} ({})", filename, detect::runner_command(self.runner))
    }
}

fn select_runner(detections: Vec<detect::Detection>) -> Result<Option<detect::Runner>, RtError> {
    let items: Vec<RunnerItem> = detections
        .into_iter()
        .map(|detection| RunnerItem {
            runner: detection.runner,
            runner_file: detection.runner_file,
        })
        .collect();

    match inquire::Select::new("Select runner", items).prompt() {
        Ok(item) => Ok(Some(item.runner)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(err) => Err(RtError::Prompt(err)),
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
