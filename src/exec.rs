use std::process::Command;

use crate::RtError;
use crate::detect::{Runner, runner_command};

pub fn run(runner: Runner, task: &str, passthrough: &[String]) -> Result<i32, RtError> {
    let mut command = base_command(runner)?;
    if runner == Runner::Mise {
        command.arg("run");
    }
    let current_dir = std::env::current_dir().map_err(RtError::Io)?;
    let status = command
        .arg(task)
        .args(passthrough)
        .current_dir(current_dir)
        .status()
        .map_err(RtError::Spawn)?;

    Ok(status.code().unwrap_or(2))
}

pub fn base_command(runner: Runner) -> Result<Command, RtError> {
    let program = runner_command(runner);
    ensure_tool(program)?;
    let mut command = Command::new(program);
    if runner == Runner::CargoMake {
        command.arg("make");
    }
    Ok(command)
}

pub fn ensure_tool(tool: &'static str) -> Result<(), RtError> {
    match which::which(tool) {
        Ok(_) => Ok(()),
        Err(_) => Err(RtError::ToolMissing { tool }),
    }
}
