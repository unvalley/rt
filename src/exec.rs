use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::RtError;
use crate::detect::{Runner, runner_command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub command: String,
}

pub fn run(
    runner: Runner,
    task: &str,
    passthrough: &[String],
    cwd: &Path,
) -> Result<RunResult, RtError> {
    let mut command = base_command(runner)?;
    if runner == Runner::Mise {
        command.arg("run");
    }
    let command_preview = preview_command(runner, task, passthrough);
    let started_at = Instant::now();
    let status = command
        .arg(task)
        .args(passthrough)
        .current_dir(cwd)
        .status()
        .map_err(RtError::Spawn)?;

    Ok(RunResult {
        exit_code: status.code().unwrap_or(2),
        duration_ms: started_at.elapsed().as_millis() as u64,
        command: command_preview,
    })
}

pub fn run_command_line(command: &str, cwd: &Path) -> Result<RunResult, RtError> {
    let argv = shell_words::split(command).map_err(|err| RtError::InvalidCommandLine {
        message: err.to_string(),
    })?;
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| RtError::InvalidCommandLine {
            message: "empty command".to_string(),
        })?;

    if !program.contains('/') && which::which(program).is_err() {
        return Err(RtError::ToolMissingCommand {
            tool: program.clone(),
        });
    }

    let started_at = Instant::now();
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(RtError::Spawn)?;

    Ok(RunResult {
        exit_code: status.code().unwrap_or(2),
        duration_ms: started_at.elapsed().as_millis() as u64,
        command: command.to_string(),
    })
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

pub fn preview_command(runner: Runner, task: &str, passthrough: &[String]) -> String {
    let mut parts = Vec::new();
    parts.push(runner_command(runner).to_string());
    if runner == Runner::CargoMake {
        parts.push("make".to_string());
    }
    if runner == Runner::Mise {
        parts.push("run".to_string());
    }
    parts.push(task.to_string());
    parts.extend(passthrough.iter().cloned());

    parts
        .into_iter()
        .map(|part| quote_shell_arg(&part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_shell_arg(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.chars().any(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '\'' | '"' | '\\' | '$' | '`' | '!' | '&' | '|' | ';' | '<' | '>'
            )
    }) {
        return format!("'{}'", value.replace('\'', "'\\''"));
    }
    value.to_string()
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

    #[test]
    fn format_command_preview_renders_simple_command() {
        let preview = preview_command(Runner::Justfile, "test", &["--verbose".to_string()]);
        assert_eq!(preview, "just test --verbose");
    }

    #[test]
    fn format_command_preview_quotes_special_args() {
        let preview = preview_command(
            Runner::Justfile,
            "test",
            &[
                "hello world".to_string(),
                "a'b".to_string(),
                "$HOME".to_string(),
            ],
        );
        assert_eq!(preview, "just test 'hello world' 'a'\\''b' '$HOME'");
    }

    #[test]
    fn preview_command_handles_runner_specific_prefixes() {
        assert_eq!(
            preview_command(Runner::Mise, "build", &[]),
            "mise run build"
        );
        assert_eq!(
            preview_command(Runner::CargoMake, "build", &[]),
            "cargo make build"
        );
    }

    #[test]
    fn run_command_line_returns_success_exit_code() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_command_line("true", &cwd).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn run_command_line_returns_command_exit_code() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_command_line("false", &cwd).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn run_command_line_parses_quoted_arguments() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_command_line("true 'hello world'", &cwd).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn run_command_line_rejects_empty_command() {
        let cwd = std::env::current_dir().unwrap();
        let err = run_command_line("   ", &cwd).unwrap_err();
        match err {
            RtError::InvalidCommandLine { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
