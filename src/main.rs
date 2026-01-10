mod cli;
mod detect;
mod error;
mod exec;

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
    exec::run(detection.runner, &cli.task, &cli.passthrough)
}

fn classify_error(err: &RiError) -> i32 {
    match err {
        RiError::NoRunnerFound { .. } | RiError::ToolMissing { .. } => 3,
        RiError::Io(_) | RiError::Spawn(_) => 2,
    }
}
