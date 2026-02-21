mod detect;
mod exec;
mod history;
mod parser;
mod task_args;
mod tasks;

use bpaf::Bpaf;
use inquire::error::InquireError;
use std::fmt;
use std::path::{Path, PathBuf};

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
    /// Prompt for task arguments interactively.
    #[bpaf(long("args"), switch)]
    prompt_args: bool,
    /// Select a previously executed command from rt history and run it.
    #[bpaf(long("history"), switch)]
    history: bool,
    /// Show verbose logs.
    #[bpaf(long("verbose"), switch)]
    verbose: bool,
    /// Task name to run in your task runner files (e.g. `build`, `test`).
    #[bpaf(positional("task"))]
    task: Option<String>,
    #[bpaf(positional("passthrough"), many)]
    rest: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    pub prompt_args: bool,
    pub history: bool,
    pub verbose: bool,
    pub task: Option<String>,
    pub passthrough: Vec<String>,
}

pub fn parse_cli() -> Cli {
    let raw = args().run();
    Cli::from_raw(raw)
}

impl Cli {
    fn from_raw(raw: Args) -> Self {
        Self {
            prompt_args: raw.prompt_args,
            history: raw.history,
            verbose: raw.verbose,
            task: raw.task,
            passthrough: normalize_passthrough(raw.rest),
        }
    }
}

fn normalize_passthrough(rest: Vec<String>) -> Vec<String> {
    match rest.split_first() {
        Some((first, rest)) if first == "--" => rest.to_vec(),
        Some((_first, _rest)) => rest,
        None => Vec::new(),
    }
}

/// Runs tasks based on the provided CLI arguments.
fn run(cli: Cli) -> Result<i32, RtError> {
    let cwd = std::env::current_dir().map_err(RtError::Io)?;
    if cli.history {
        return rerun_from_history(&cwd, cli.verbose);
    }

    if let Some(task) = cli.task {
        let detection = detect::detect_runner(&cwd)?;
        let passthrough =
            match collect_passthrough(&detection, &task, &cli.passthrough, cli.prompt_args)? {
                Some(args) => args,
                None => return Ok(0),
            };
        return execute_and_record(&detection, &task, &passthrough, &cwd, cli.verbose);
    }

    let detections = detect::detect_runners(&cwd)?;
    let detection = if detections.len() == 1 {
        detections.into_iter().next()
    } else {
        select_runner(detections)?
    };

    let detection = match detection {
        Some(detection) => detection,
        None => return Ok(0),
    };
    let runner = detection.runner;

    let task = tasks::select_task(runner)?;
    match task {
        Some(task) => {
            let passthrough =
                match collect_passthrough(&detection, &task, &cli.passthrough, cli.prompt_args)? {
                    Some(args) => args,
                    None => return Ok(0),
                };
            execute_and_record(&detection, &task, &passthrough, &cwd, cli.verbose)
        }
        None => Ok(0),
    }
}

const HISTORY_SELECT_LIMIT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryChoice {
    ts: String,
    exit_code: i32,
    duration_ms: u64,
    cwd: String,
    cmd: String,
}

impl fmt::Display for HistoryChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}  exit={}  {}ms  {}  {}",
            format_history_timestamp(&self.ts),
            self.exit_code,
            self.duration_ms,
            self.cwd,
            self.cmd
        )
    }
}

fn rerun_from_history(fallback_cwd: &Path, verbose: bool) -> Result<i32, RtError> {
    let records = history::read_default().map_err(RtError::Io)?;
    let choices = build_history_choices(&records, HISTORY_SELECT_LIMIT);
    if choices.is_empty() {
        if verbose {
            eprintln!("rt history is empty");
        }
        return Ok(0);
    }

    let selected = match inquire::Select::new("Select history command", choices).prompt() {
        Ok(item) => item,
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => return Ok(0),
        Err(err) => return Err(RtError::Prompt(err)),
    };

    let execution_cwd = resolve_history_cwd(&selected.cwd, fallback_cwd);
    if verbose && execution_cwd != selected.cwd {
        eprintln!(
            "history cwd not found, falling back to current directory: {}",
            execution_cwd.to_string_lossy()
        );
    }

    let result = exec::run_shell(&selected.cmd, &execution_cwd)?;
    if let Err(err) = history::append_shell_default(history::ShellRecordInput {
        command: &result.command,
        cwd: &execution_cwd,
        exit_code: result.exit_code,
        duration_ms: result.duration_ms,
    }) && verbose
    {
        eprintln!("failed to write rt history: {err}");
    }

    Ok(result.exit_code)
}

fn build_history_choices(records: &[history::StoredRecord], limit: usize) -> Vec<HistoryChoice> {
    records
        .iter()
        .rev()
        .take(limit)
        .map(|entry| HistoryChoice {
            ts: entry.record.ts.clone(),
            exit_code: entry.record.exit_code,
            duration_ms: entry.record.duration_ms,
            cwd: entry.record.cwd.clone(),
            cmd: entry.record.cmd.clone(),
        })
        .collect()
}

fn resolve_history_cwd(recorded_cwd: &str, fallback_cwd: &Path) -> PathBuf {
    let candidate = PathBuf::from(recorded_cwd);
    if candidate.is_dir() {
        candidate
    } else {
        fallback_cwd.to_path_buf()
    }
}

fn format_history_timestamp(ts: &str) -> String {
    if ts.len() >= 19 {
        ts[..19].replace('T', " ")
    } else {
        ts.to_string()
    }
}

fn execute_and_record(
    detection: &detect::Detection,
    task: &str,
    passthrough: &[String],
    cwd: &Path,
    verbose: bool,
) -> Result<i32, RtError> {
    let result = exec::run(detection.runner, task, passthrough, cwd)?;
    if let Err(err) = history::append_default(history::RecordInput {
        runner: detection.runner,
        command: &result.command,
        task,
        cwd,
        exit_code: result.exit_code,
        duration_ms: result.duration_ms,
        runner_file: Some(detection.runner_file.as_path()),
    }) && verbose
    {
        eprintln!("failed to write rt history: {err}");
    }

    Ok(result.exit_code)
}

fn collect_passthrough(
    detection: &detect::Detection,
    task: &str,
    cli_passthrough: &[String],
    prompt_optional_args: bool,
) -> Result<Option<Vec<String>>, RtError> {
    let required = task_args::required_args_for_task(detection, task).map_err(RtError::Io)?;
    let plan = build_passthrough_plan(&required, cli_passthrough, prompt_optional_args);
    let mut passthrough = plan.initial_passthrough;

    if plan.missing_required.is_empty() && !plan.prompt_optional_args {
        return Ok(Some(passthrough));
    }

    for name in &plan.missing_required {
        let value = match prompt_required_argument(detection.runner, task, name, &passthrough)? {
            Some(value) => value,
            None => return Ok(None),
        };
        passthrough.push(value);
    }

    if plan.prompt_optional_args {
        let optional = match prompt_optional_passthrough(detection.runner, task, &passthrough)? {
            Some(args) => args,
            None => return Ok(None),
        };
        passthrough.extend(optional);
    }

    Ok(Some(passthrough))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PassthroughPlan {
    initial_passthrough: Vec<String>,
    missing_required: Vec<String>,
    prompt_optional_args: bool,
}

fn build_passthrough_plan(
    required: &[String],
    cli_passthrough: &[String],
    prompt_optional_args: bool,
) -> PassthroughPlan {
    let start = cli_passthrough.len().min(required.len());
    PassthroughPlan {
        initial_passthrough: cli_passthrough.to_vec(),
        missing_required: required[start..].to_vec(),
        prompt_optional_args,
    }
}

fn prompt_required_argument(
    runner: detect::Runner,
    task: &str,
    name: &str,
    current: &[String],
) -> Result<Option<String>, RtError> {
    loop {
        let message = format!("Value for required arg {name}");
        let preview = exec::preview_command(runner, task, current);
        match inquire::Text::new(&message)
            .with_help_message(&format!("Current: $ {preview}"))
            .prompt()
        {
            Ok(input) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    eprintln!("Argument `{name}` is required. Enter a value or cancel.");
                    continue;
                }
                return Ok(Some(trimmed.to_string()));
            }
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                return Ok(None);
            }
            Err(err) => return Err(RtError::Prompt(err)),
        }
    }
}

fn prompt_optional_passthrough(
    runner: detect::Runner,
    task: &str,
    current: &[String],
) -> Result<Option<Vec<String>>, RtError> {
    let preview = exec::preview_command(runner, task, current);
    let message = format!("Additional arguments for {task} (optional, space-separated)");
    match inquire::Text::new(&message)
        .with_help_message(&format!("Current: $ {preview}"))
        .prompt()
    {
        Ok(input) => Ok(Some(split_interactive_passthrough(&input))),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(err) => Err(RtError::Prompt(err)),
    }
}

fn split_interactive_passthrough(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|arg| arg.to_string())
        .collect()
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
    detection: detect::Detection,
}

impl fmt::Display for RunnerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let filename = self
            .detection
            .runner_file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.detection.runner_file.to_string_lossy().into_owned());
        write!(
            f,
            "{} ({})",
            filename,
            detect::runner_command(self.detection.runner)
        )
    }
}

fn select_runner(detections: Vec<detect::Detection>) -> Result<Option<detect::Detection>, RtError> {
    let items: Vec<RunnerItem> = detections
        .into_iter()
        .map(|detection| RunnerItem { detection })
        .collect();

    match inquire::Select::new("Select runner", items).prompt() {
        Ok(item) => Ok(Some(item.detection)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_passthrough_strips_separator_only_when_first() {
        assert_eq!(
            normalize_passthrough(vec!["--".into(), "foo".into(), "--bar".into()]),
            vec!["foo".to_string(), "--bar".to_string()]
        );
        assert_eq!(
            normalize_passthrough(vec!["foo".into(), "--".into(), "bar".into()]),
            vec!["foo".to_string(), "--".to_string(), "bar".to_string()]
        );
        assert!(normalize_passthrough(Vec::new()).is_empty());
    }

    #[test]
    fn classify_error_returns_expected_exit_codes() {
        let cwd = PathBuf::from(".");
        assert_eq!(classify_error(&RtError::NoRunnerFound { cwd }), 3);
        assert_eq!(classify_error(&RtError::ToolMissing { tool: "just" }), 3);
        assert_eq!(classify_error(&RtError::NoTasks { tool: "just" }), 3);
        assert_eq!(
            classify_error(&RtError::ListFailed {
                tool: "just",
                status: 1
            }),
            3
        );
        assert_eq!(
            classify_error(&RtError::Io(std::io::Error::from(
                std::io::ErrorKind::Other
            ))),
            2
        );
    }

    #[test]
    fn split_interactive_passthrough_handles_whitespace() {
        assert_eq!(
            split_interactive_passthrough("foo  bar --baz"),
            vec!["foo".to_string(), "bar".to_string(), "--baz".to_string()]
        );
        assert!(split_interactive_passthrough("").is_empty());
        assert!(split_interactive_passthrough("   ").is_empty());
    }

    #[test]
    fn prompt_passthrough_prefers_cli_passthrough() {
        let detection = detect::Detection {
            runner: detect::Runner::Taskfile,
            runner_file: PathBuf::from("Taskfile.yml"),
        };
        let passthrough = vec!["--flag".to_string(), "value".to_string()];
        let result = collect_passthrough(&detection, "build", &passthrough, false)
            .unwrap()
            .unwrap();
        assert_eq!(result, passthrough);
    }

    #[test]
    fn cli_from_raw_parses_args_flag_and_passthrough() {
        let raw = Args {
            prompt_args: true,
            history: true,
            verbose: true,
            task: Some("build".to_string()),
            rest: vec!["--".to_string(), "--env".to_string(), "prod".to_string()],
        };
        let cli = Cli::from_raw(raw);
        assert!(cli.prompt_args);
        assert!(cli.history);
        assert!(cli.verbose);
        assert_eq!(cli.task.as_deref(), Some("build"));
        assert_eq!(
            cli.passthrough,
            vec!["--env".to_string(), "prod".to_string()]
        );
    }

    #[test]
    fn build_history_choices_returns_newest_first_with_limit() {
        let records = vec![
            history::StoredRecord {
                raw: "a".to_string(),
                record: history::HistoryRecord {
                    v: 1,
                    ts: "2026-02-21T12:00:00+09:00".to_string(),
                    cmd: "make a".to_string(),
                    cwd: "/repo".to_string(),
                    exit_code: 0,
                    duration_ms: 10,
                    engine: None,
                    target: None,
                    file: None,
                    hostname: None,
                    user: None,
                },
            },
            history::StoredRecord {
                raw: "b".to_string(),
                record: history::HistoryRecord {
                    v: 1,
                    ts: "2026-02-21T12:01:00+09:00".to_string(),
                    cmd: "make b".to_string(),
                    cwd: "/repo".to_string(),
                    exit_code: 1,
                    duration_ms: 20,
                    engine: None,
                    target: None,
                    file: None,
                    hostname: None,
                    user: None,
                },
            },
        ];

        let choices = build_history_choices(&records, 1);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].cmd, "make b");
        assert_eq!(choices[0].exit_code, 1);
    }

    #[test]
    fn resolve_history_cwd_falls_back_when_recorded_path_is_missing() {
        let fallback = std::env::current_dir().unwrap();
        let resolved = resolve_history_cwd("/__definitely_missing__/rt", &fallback);
        assert_eq!(resolved, fallback);
    }

    #[test]
    fn format_history_timestamp_truncates_rfc3339() {
        assert_eq!(
            format_history_timestamp("2026-02-21T12:34:56+09:00"),
            "2026-02-21 12:34:56".to_string()
        );
    }

    #[test]
    fn build_passthrough_plan_without_args_flag_and_no_required() {
        let required = Vec::<String>::new();
        let cli = vec!["--verbose".to_string()];
        let plan = build_passthrough_plan(&required, &cli, false);
        assert_eq!(
            plan,
            PassthroughPlan {
                initial_passthrough: vec!["--verbose".to_string()],
                missing_required: Vec::new(),
                prompt_optional_args: false,
            }
        );
    }

    #[test]
    fn build_passthrough_plan_with_args_flag_prompts_optional() {
        let required = Vec::<String>::new();
        let cli = vec!["--verbose".to_string()];
        let plan = build_passthrough_plan(&required, &cli, true);
        assert_eq!(
            plan,
            PassthroughPlan {
                initial_passthrough: vec!["--verbose".to_string()],
                missing_required: Vec::new(),
                prompt_optional_args: true,
            }
        );
    }

    #[test]
    fn build_passthrough_plan_detects_missing_required_args() {
        let required = vec!["ENV".to_string(), "TARGET".to_string()];
        let cli = vec!["prod".to_string()];
        let plan = build_passthrough_plan(&required, &cli, false);
        assert_eq!(
            plan,
            PassthroughPlan {
                initial_passthrough: vec!["prod".to_string()],
                missing_required: vec!["TARGET".to_string()],
                prompt_optional_args: false,
            }
        );
    }
}
