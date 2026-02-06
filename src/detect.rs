use std::path::{Path, PathBuf};

use crate::RtError;

const RUNNER_CANDIDATES: [(&str, Runner); 15] = [
    ("Justfile", Runner::Justfile),
    ("justfile", Runner::Justfile),
    ("Taskfile.yml", Runner::Taskfile),
    ("taskfile.yml", Runner::Taskfile),
    ("Taskfile.yaml", Runner::Taskfile),
    ("taskfile.yaml", Runner::Taskfile),
    ("Taskfile.dist.yml", Runner::Taskfile),
    ("taskfile.dist.yml", Runner::Taskfile),
    ("Taskfile.dist.yaml", Runner::Taskfile),
    ("taskfile.dist.yaml", Runner::Taskfile),
    ("maskfile.md", Runner::Maskfile),
    ("Maskfile.md", Runner::Maskfile),
    ("mise.toml", Runner::Mise),
    ("Makefile.toml", Runner::CargoMake),
    ("Makefile", Runner::Makefile),
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Runner {
    Justfile,
    Taskfile,
    Maskfile,
    Mise,
    CargoMake,
    Makefile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub runner: Runner,
    pub runner_file: PathBuf,
}

/// Detects the task runner used in the given directory.
pub fn detect_runner(dir_path: &Path) -> Result<Detection, RtError> {
    for (name, runner) in RUNNER_CANDIDATES {
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

/// Detects all available runners in the given directory, in priority order.
pub fn detect_runners(dir_path: &Path) -> Result<Vec<Detection>, RtError> {
    let mut seen = std::collections::HashSet::new();
    let mut detections = Vec::new();

    for (name, runner) in RUNNER_CANDIDATES {
        if seen.contains(&runner) {
            continue;
        }
        let path = dir_path.join(name);
        if path.is_file() {
            seen.insert(runner);
            detections.push(Detection {
                runner,
                runner_file: path,
            });
        }
    }

    if detections.is_empty() {
        Err(RtError::NoRunnerFound {
            cwd: dir_path.to_path_buf(),
        })
    } else {
        Ok(detections)
    }
}

/// Returns the command name for the given runner.
pub fn runner_command(runner: Runner) -> &'static str {
    match runner {
        Runner::Justfile => "just",
        Runner::Taskfile => "task",
        Runner::Maskfile => "mask",
        Runner::Mise => "mise",
        // cargo-make is a subcommand of cargo, so we need to check cargo
        Runner::CargoMake => "cargo",
        Runner::Makefile => "make",
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
        touch(dir.path(), "maskfile.md");
        touch(dir.path(), "Taskfile.yml");
        let just_path = touch(dir.path(), "justfile");

        let detection = detect_runner(dir.path()).unwrap();
        assert_eq!(detection.runner, Runner::Justfile);
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

    #[test]
    fn runner_command_mapping() {
        assert_eq!(runner_command(Runner::Justfile), "just");
        assert_eq!(runner_command(Runner::Taskfile), "task");
        assert_eq!(runner_command(Runner::Maskfile), "mask");
        assert_eq!(runner_command(Runner::Mise), "mise");
        assert_eq!(runner_command(Runner::CargoMake), "cargo");
        assert_eq!(runner_command(Runner::Makefile), "make");
    }

    #[test]
    fn detect_runners_returns_all_in_priority_order() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "Makefile");
        touch(dir.path(), "Makefile.toml");
        touch(dir.path(), "mise.toml");
        touch(dir.path(), "maskfile.md");
        touch(dir.path(), "Taskfile.yml");
        touch(dir.path(), "justfile");

        let detections = detect_runners(dir.path()).unwrap();
        let runners: Vec<Runner> = detections.into_iter().map(|d| d.runner).collect();

        assert_eq!(
            runners,
            vec![
                Runner::Justfile,
                Runner::Taskfile,
                Runner::Maskfile,
                Runner::Mise,
                Runner::CargoMake,
                Runner::Makefile,
            ]
        );
    }

    #[test]
    fn detect_runners_deduplicates_case_variants() {
        let dir = tempdir().unwrap();
        touch(dir.path(), "Justfile");
        touch(dir.path(), "justfile");
        touch(dir.path(), "Taskfile.yml");
        touch(dir.path(), "taskfile.yaml");

        let detections = detect_runners(dir.path()).unwrap();
        let runners: Vec<Runner> = detections.into_iter().map(|d| d.runner).collect();

        assert_eq!(runners, vec![Runner::Justfile, Runner::Taskfile]);
    }
}
