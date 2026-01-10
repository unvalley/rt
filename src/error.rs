use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum RiError {
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
