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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_command_for_cargo_make_includes_make_subcommand() {
        let command = base_command(Runner::CargoMake).unwrap();
        assert_eq!(command.get_program(), "cargo");
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["make".to_string()]);
    }

    #[test]
    fn ensure_tool_returns_error_for_missing_binary() {
        let err = ensure_tool("__rt_missing_tool_for_test__").unwrap_err();
        match err {
            RtError::ToolMissing { tool } => assert_eq!(tool, "__rt_missing_tool_for_test__"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
