use std::path::{Path, PathBuf};

use crate::RtError;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Runner {
    Just,
    Taskfile,
    CargoMake,
    Make,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub runner: Runner,
    pub runner_file: PathBuf,
}

/// Detects the task runner used in the given directory.
pub fn detect_runner(dir_path: &Path) -> Result<Detection, RtError> {
    let candidates: [(&str, Runner); 12] = [
        ("Justfile", Runner::Just),
        ("justfile", Runner::Just),
        ("Taskfile.yml", Runner::Taskfile),
        ("taskfile.yml", Runner::Taskfile),
        ("Taskfile.yaml", Runner::Taskfile),
        ("taskfile.yaml", Runner::Taskfile),
        ("Taskfile.dist.yml", Runner::Taskfile),
        ("taskfile.dist.yml", Runner::Taskfile),
        ("Taskfile.dist.yaml", Runner::Taskfile),
        ("taskfile.dist.yaml", Runner::Taskfile),
        ("Makefile.toml", Runner::CargoMake),
        ("Makefile", Runner::Make),
    ];

    for (name, runner) in candidates {
        let path = dir_path.join(name);
        if path.is_file() {
            return Ok(Detection {
                runner,
                runner_file: path,
            });
        }
    }

    Err(RtError::NoRunnerFound {
        cwd: dir_path.to_path_buf(),
    })
}

pub fn runner_command(runner: Runner) -> &'static str {
    match runner {
        Runner::Just => "just",
        Runner::Taskfile => "task",
        Runner::CargoMake => "cargo",
        Runner::Make => "make",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"").unwrap();
        path
    }

    #[test]
    fn detect_none_returns_error() {
        let dir = tempdir().unwrap();
        let err = detect_runner(dir.path()).unwrap_err();
        match err {
            RtError::NoRunnerFound { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn detect_prefers_justfile_over_others() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "Makefile");
        touch(dir.path(), "Makefile.toml");
        touch(dir.path(), "Taskfile.yml");
        let just_path = touch(dir.path(), "justfile");

        let detection = detect_runner(dir.path()).unwrap();
        assert_eq!(detection.runner, Runner::Just);
        let name = detection
            .runner_file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap();
        assert!(name.eq_ignore_ascii_case("justfile"));
        assert!(
            detection
                .runner_file
                .parent()
                .is_some_and(|p| p == dir.path())
        );
        assert!(just_path.exists());
    }

    #[test]
    fn detect_prefers_taskfile_yml_over_yaml() {
        let dir = tempdir().unwrap();
        let yml = touch(dir.path(), "Taskfile.yml");
        touch(dir.path(), "Taskfile.yaml");

        let detection = detect_runner(dir.path()).unwrap();
        assert_eq!(detection.runner, Runner::Taskfile);
        assert_eq!(detection.runner_file, yml);
    }
}
