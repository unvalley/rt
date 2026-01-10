use std::process::Command;

use crate::detect::{Runner, runner_command};
use crate::error::RiError;

pub fn run(runner: Runner, task: &str, passthrough: &[String]) -> Result<i32, RiError> {
    let program = runner_command(runner);
    ensure_tool(program)?;

    let mut command = make_command(runner);

    let current_dir = std::env::current_dir().map_err(RiError::Io)?;
    let status = command
        .arg(task)
        .args(passthrough)
        .current_dir(current_dir)
        .status()
        .map_err(RiError::Spawn)?;

    Ok(status.code().unwrap_or(2))
}

fn make_command(runner: Runner) -> Command {
    let program = runner_command(runner);
    let mut command = Command::new(program);
    if runner == Runner::CargoMake {
        command.arg("make");
    }
    command
}

fn ensure_tool(tool: &'static str) -> Result<(), RiError> {
    match which::which(tool) {
        Ok(_) => Ok(()),
        Err(_) => Err(RiError::ToolMissing { tool }),
    }
}
