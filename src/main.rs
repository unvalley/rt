mod cli;
mod detect;
mod error;
mod exec;
mod tasks;

use crate::error::RtError;

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
        RtError::NoRunnerFound { .. } | RtError::ToolMissing { .. } | RtError::NoTasks { .. } => 3,
        RtError::ListFailed { .. } | RtError::Prompt(_) | RtError::Io(_) | RtError::Spawn(_) => 2,
    }
}
