use std::path::{Path, PathBuf};

use crate::RtError;

const RUNNER_CANDIDATES: [(&str, Runner); 22] = [
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
    ("vite.config.ts", Runner::VitePlus),
    ("vite.config.mts", Runner::VitePlus),
    ("vite.config.cts", Runner::VitePlus),
    ("vite.config.js", Runner::VitePlus),
    ("vite.config.mjs", Runner::VitePlus),
    ("vite.config.cjs", Runner::VitePlus),
    ("package.json", Runner::VitePlus),
    ("mise.toml", Runner::Mise),
    ("Makefile.toml", Runner::CargoMake),
    ("Makefile", Runner::Makefile),
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Runner {
    Justfile,
    Taskfile,
    Maskfile,
    VitePlus,
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
        if candidate_matches(runner, &path) {
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
        if candidate_matches(runner, &path) {
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

fn candidate_matches(runner: Runner, path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    match runner {
        Runner::VitePlus => vite_plus_marker_matches(path),
        _ => true,
    }
}

fn vite_plus_marker_matches(path: &Path) -> bool {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("package.json") => package_json_declares_vite_plus(path),
        Some(name) if name.starts_with("vite.config.") => vite_config_declares_vite_plus(path),
        _ => true,
    }
}

fn vite_config_declares_vite_plus(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.contains("vite-plus"))
        .unwrap_or(false)
}

fn package_json_declares_vite_plus(path: &Path) -> bool {
    #[derive(serde::Deserialize)]
    struct PackageJson {
        #[serde(default)]
        dependencies: std::collections::BTreeMap<String, serde_json::Value>,
        #[serde(default, rename = "devDependencies")]
        dev_dependencies: std::collections::BTreeMap<String, serde_json::Value>,
        #[serde(default, rename = "peerDependencies")]
        peer_dependencies: std::collections::BTreeMap<String, serde_json::Value>,
        #[serde(default, rename = "optionalDependencies")]
        optional_dependencies: std::collections::BTreeMap<String, serde_json::Value>,
    }

    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(package_json) = serde_json::from_str::<PackageJson>(&content) else {
        return false;
    };

    [
        &package_json.dependencies,
        &package_json.dev_dependencies,
        &package_json.peer_dependencies,
        &package_json.optional_dependencies,
    ]
    .into_iter()
    .any(|deps| deps.contains_key("vite-plus"))
}

/// Returns the command name for the given runner.
pub fn runner_command(runner: Runner) -> &'static str {
    match runner {
        Runner::Justfile => "just",
        Runner::Taskfile => "task",
        Runner::Maskfile => "mask",
        Runner::VitePlus => "vp",
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

    fn write(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
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
    fn detect_ignores_plain_vite_config_without_vite_plus_marker() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "vite.config.ts",
            "import { defineConfig } from 'vite'; export default defineConfig({});",
        );

        let err = detect_runner(dir.path()).unwrap_err();
        match err {
            RtError::NoRunnerFound { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn detect_package_json_with_vite_plus_dependency() {
        let dir = tempdir().unwrap();
        let package_json = write(
            dir.path(),
            "package.json",
            r#"{"devDependencies":{"vite-plus":"^1.0.0"}}"#,
        );

        let detection = detect_runner(dir.path()).unwrap();
        assert_eq!(detection.runner, Runner::VitePlus);
        assert_eq!(detection.runner_file, package_json);
    }

    #[test]
    fn runner_command_mapping() {
        assert_eq!(runner_command(Runner::Justfile), "just");
        assert_eq!(runner_command(Runner::Taskfile), "task");
        assert_eq!(runner_command(Runner::Maskfile), "mask");
        assert_eq!(runner_command(Runner::VitePlus), "vp");
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
        write(
            dir.path(),
            "vite.config.ts",
            "import { defineConfig } from 'vite-plus'; export default defineConfig({});",
        );
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
                Runner::VitePlus,
                Runner::Mise,
                Runner::CargoMake,
                Runner::Makefile,
            ]
        );
    }

    #[test]
    fn detect_runners_deduplicates_vite_plus_config_variants() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "vite.config.ts",
            "import { defineConfig } from 'vite-plus'; export default defineConfig({});",
        );
        write(
            dir.path(),
            "vite.config.mjs",
            "import { defineConfig } from 'vite-plus'; export default defineConfig({});",
        );

        let detections = detect_runners(dir.path()).unwrap();
        let runners: Vec<Runner> = detections.into_iter().map(|d| d.runner).collect();

        assert_eq!(runners, vec![Runner::VitePlus]);
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
