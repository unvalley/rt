mod cli;
mod detect;
mod error;
mod exec;
mod tasks;

use crate::error::RiError;

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

fn run(cli: cli::Cli) -> Result<i32, RiError> {
    let cwd = std::env::current_dir().map_err(RiError::Io)?;
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

fn classify_error(err: &RiError) -> i32 {
    match err {
        RiError::NoRunnerFound { .. }
        | RiError::ToolMissing { .. }
        | RiError::NoTasks { .. } => 3,
        RiError::ListFailed { .. } | RiError::Prompt(_) | RiError::Io(_) | RiError::Spawn(_) => 2,
    }
}
