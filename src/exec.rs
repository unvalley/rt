use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::RtError;
use crate::detect::{Runner, runner_command};

pub fn run(runner: Runner, task: &str, passthrough: &[String]) -> Result<i32, RtError> {
    let mut command = base_command(runner)?;
    if runner == Runner::Mise {
        command.arg("run");
    }
    let current_dir = std::env::current_dir().map_err(RtError::Io)?;
    command.arg(task).args(passthrough).current_dir(current_dir);
    append_command_to_shell_history(&command);
    let status = command.status().map_err(RtError::Spawn)?;

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

fn append_command_to_shell_history(command: &Command) {
    let Some(histfile) = std::env::var_os("HISTFILE") else {
        return;
    };
    let shell = std::env::var("SHELL").ok();
    let command_line = command_to_shell_string(command);
    if command_line.trim().is_empty() {
        return;
    }

    let format = history_format(shell.as_deref(), Path::new(&histfile));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let line = format_history_entry(format, &command_line, now);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(histfile) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn command_to_shell_string(command: &Command) -> String {
    let mut parts = Vec::new();
    parts.push(shell_escape(&sanitize_arg(command.get_program())));
    parts.extend(
        command
            .get_args()
            .map(|arg| shell_escape(&sanitize_arg(arg))),
    );
    parts.join(" ")
}

fn sanitize_arg(value: &OsStr) -> String {
    value.to_string_lossy().replace(['\n', '\r'], " ")
}

fn shell_escape(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .bytes()
        .all(|ch| ch.is_ascii_alphanumeric() || b"@%_+=:,./-".contains(&ch))
    {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryFormat {
    Plain,
    ZshExtended,
}

fn history_format(shell: Option<&str>, histfile: &Path) -> HistoryFormat {
    if shell
        .and_then(|shell| Path::new(shell).file_name())
        .and_then(|name| name.to_str())
        != Some("zsh")
    {
        return HistoryFormat::Plain;
    }
    if is_zsh_extended_history(histfile) {
        HistoryFormat::ZshExtended
    } else {
        HistoryFormat::Plain
    }
}

fn is_zsh_extended_history(histfile: &Path) -> bool {
    let file = match std::fs::File::open(histfile) {
        Ok(file) => file,
        Err(_) => return false,
    };
    for line in BufReader::new(file).lines().map_while(Result::ok).take(20) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return trimmed.starts_with(": ") && trimmed.contains(';');
    }
    false
}

fn format_history_entry(format: HistoryFormat, command_line: &str, unix_time: u64) -> String {
    match format {
        HistoryFormat::Plain => format!("{command_line}\n"),
        HistoryFormat::ZshExtended => format!(": {unix_time}:0;{command_line}\n"),
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
    use std::process::Command;
    use tempfile::tempdir;

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
    fn command_to_shell_string_quotes_special_chars() {
        let mut command = Command::new("just");
        command
            .arg("test-all")
            .arg("--name=O'Reilly")
            .arg("two words");

        let line = command_to_shell_string(&command);
        assert_eq!(line, "just test-all '--name=O'\"'\"'Reilly' 'two words'");
    }

    #[test]
    fn command_to_shell_string_sanitizes_newlines() {
        let mut command = Command::new("task");
        command.arg("run\nname").arg("line\rbreak");

        let line = command_to_shell_string(&command);
        assert_eq!(line, "task 'run name' 'line break'");
    }

    #[test]
    fn format_history_entry_for_plain() {
        let line = format_history_entry(HistoryFormat::Plain, "just build", 1234);
        assert_eq!(line, "just build\n");
    }

    #[test]
    fn format_history_entry_for_zsh_extended() {
        let line = format_history_entry(HistoryFormat::ZshExtended, "just build", 1234);
        assert_eq!(line, ": 1234:0;just build\n");
    }

    #[test]
    fn history_format_detects_zsh_extended_history() {
        let dir = tempdir().unwrap();
        let hist = dir.path().join(".zsh_history");
        std::fs::write(&hist, ": 1738896400:0;ls -la\n").unwrap();

        assert_eq!(
            history_format(Some("/bin/zsh"), &hist),
            HistoryFormat::ZshExtended
        );
    }

    #[test]
    fn history_format_defaults_to_plain_for_non_zsh() {
        let dir = tempdir().unwrap();
        let hist = dir.path().join(".history");
        std::fs::write(&hist, ": 1738896400:0;ls -la\n").unwrap();

        assert_eq!(
            history_format(Some("/bin/bash"), &hist),
            HistoryFormat::Plain
        );
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
}
