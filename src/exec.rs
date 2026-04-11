use std::path::Path;
use std::process::Command;

use crate::RtError;
use crate::detect::{Runner, runner_command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub exit_code: i32,
    pub program: String,
    pub args: Vec<String>,
}

pub fn run(
    runner: Runner,
    task: &str,
    passthrough: &[String],
    cwd: &Path,
) -> Result<RunResult, RtError> {
    let program = runner_command(runner).to_string();
    let mut args = runner_prefix_args(runner);
    args.push(task.to_string());
    args.extend(passthrough.iter().cloned());

    let mut command = base_command(runner)?;
    let status = command
        .arg(task)
        .args(passthrough)
        .current_dir(cwd)
        .status()
        .map_err(RtError::Spawn)?;

    Ok(RunResult {
        exit_code: status.code().unwrap_or(2),
        program,
        args,
    })
}

pub fn run_program(program: &str, args: &[String], cwd: &Path) -> Result<RunResult, RtError> {
    if !program.contains('/') && which::which(program).is_err() {
        return Err(RtError::ToolMissingCommand {
            tool: program.to_string(),
        });
    }

    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(RtError::Spawn)?;

    Ok(RunResult {
        exit_code: status.code().unwrap_or(2),
        program: program.to_string(),
        args: args.to_vec(),
    })
}

pub fn base_command(runner: Runner) -> Result<Command, RtError> {
    let program = runner_command(runner);
    ensure_tool(program)?;
    let mut command = Command::new(program);
    for arg in runner_prefix_args(runner) {
        command.arg(arg);
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
    let program = runner_command(runner);
    let mut parts = runner_prefix_args(runner);
    parts.push(task.to_string());
    parts.extend(passthrough.iter().cloned());

    format_program_args(program, &parts)
}

fn runner_prefix_args(runner: Runner) -> Vec<String> {
    match runner {
        Runner::CargoMake => vec!["make".to_string()],
        Runner::VitePlus | Runner::Mise => vec!["run".to_string()],
        _ => Vec::new(),
    }
}

pub fn format_program_args(program: &str, args: &[String]) -> String {
    let mut parts = Vec::new();
    parts.push(program.to_string());
    parts.extend(args.iter().cloned());
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
    fn runner_prefix_args_include_vite_plus_run_subcommand() {
        assert_eq!(
            runner_prefix_args(Runner::VitePlus),
            vec!["run".to_string()]
        );
    }

    #[test]
    fn runner_prefix_args_include_runner_specific_prefixes() {
        assert_eq!(
            runner_prefix_args(Runner::CargoMake),
            vec!["make".to_string()]
        );
        assert_eq!(runner_prefix_args(Runner::Mise), vec!["run".to_string()]);
        assert!(runner_prefix_args(Runner::Justfile).is_empty());
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
            preview_command(Runner::VitePlus, "build", &[]),
            "vp run build"
        );
        assert_eq!(
            preview_command(Runner::CargoMake, "build", &[]),
            "cargo make build"
        );
    }

    #[test]
    fn run_program_returns_success_exit_code() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_program("true", &[], &cwd).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn run_program_returns_command_exit_code() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_program("false", &[], &cwd).unwrap();
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn run_program_passes_arguments() {
        let cwd = std::env::current_dir().unwrap();
        let result = run_program("true", &["hello world".to_string()], &cwd).unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn format_program_args_quotes_special_args() {
        let command = format_program_args(
            "make",
            &[
                "build".to_string(),
                "hello world".to_string(),
                "a'b".to_string(),
            ],
        );
        assert_eq!(command, "make build 'hello world' 'a'\\''b'");
    }
}
