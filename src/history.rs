use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::detect::Runner;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryEngine {
    Make,
    Just,
    Shell,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub v: u8,
    pub ts: String,
    pub cmd: String,
    pub cwd: String,
    #[serde(rename = "exit")]
    pub exit_code: i32,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<HistoryEngine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

pub struct RecordInput<'a> {
    pub runner: Runner,
    pub command: &'a str,
    pub task: &'a str,
    pub cwd: &'a Path,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub runner_file: Option<&'a Path>,
}

pub struct ShellRecordInput<'a> {
    pub command: &'a str,
    pub cwd: &'a Path,
    pub exit_code: i32,
    pub duration_ms: u64,
}

impl HistoryRecord {
    pub fn from_input(input: RecordInput<'_>) -> Self {
        Self {
            v: 1,
            ts: current_timestamp(),
            cmd: input.command.to_string(),
            cwd: input.cwd.to_string_lossy().into_owned(),
            exit_code: input.exit_code,
            duration_ms: input.duration_ms,
            engine: Some(engine_for_runner(input.runner)),
            target: Some(input.task.to_string()),
            file: input
                .runner_file
                .map(|path| path.to_string_lossy().into_owned()),
            hostname: non_empty_env("HOSTNAME"),
            user: non_empty_env("USER"),
        }
    }

    pub fn from_shell_input(input: ShellRecordInput<'_>) -> Self {
        Self {
            v: 1,
            ts: current_timestamp(),
            cmd: input.command.to_string(),
            cwd: input.cwd.to_string_lossy().into_owned(),
            exit_code: input.exit_code,
            duration_ms: input.duration_ms,
            engine: Some(HistoryEngine::Shell),
            target: None,
            file: None,
            hostname: non_empty_env("HOSTNAME"),
            user: non_empty_env("USER"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecord {
    pub raw: String,
    pub record: HistoryRecord,
}

impl HistoryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, record: &HistoryRecord) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.try_lock_exclusive()?;

        let json =
            serde_json::to_string(record).map_err(|err| io::Error::other(format!("{err}")))?;
        writeln!(file, "{json}")?;
        file.flush()?;
        file.unlock()?;
        Ok(())
    }

    pub fn read_all(&self) -> io::Result<Vec<StoredRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = OpenOptions::new().read(true).open(&self.path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<HistoryRecord>(&line) {
                records.push(StoredRecord { raw: line, record });
            }
        }

        Ok(records)
    }
}

pub fn append_default(input: RecordInput<'_>) -> io::Result<()> {
    let record = HistoryRecord::from_input(input);
    append_record_default(&record)
}

pub fn append_shell_default(input: ShellRecordInput<'_>) -> io::Result<()> {
    let record = HistoryRecord::from_shell_input(input);
    append_record_default(&record)
}

fn append_record_default(record: &HistoryRecord) -> io::Result<()> {
    let candidates = default_history_paths();
    let mut last_error = None;

    for path in candidates {
        let store = HistoryStore::new(path);
        match store.append(record) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Err(err),
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error.unwrap_or_else(|| io::Error::other("failed to write history")))
}

pub fn read_default() -> io::Result<Vec<StoredRecord>> {
    read_from_paths(default_history_paths())
}

fn default_history_paths() -> Vec<PathBuf> {
    let xdg_state_home = env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);
    history_path_candidates(xdg_state_home.as_deref(), home.as_deref())
}

fn read_from_paths(paths: Vec<PathBuf>) -> io::Result<Vec<StoredRecord>> {
    let mut all_records = Vec::new();
    let mut last_error = None;

    for path in paths {
        let store = HistoryStore::new(path);
        match store.read_all() {
            Ok(mut records) => all_records.append(&mut records),
            Err(err) => last_error = Some(err),
        }
    }

    if all_records.is_empty() {
        return last_error.map_or_else(|| Ok(Vec::new()), Err);
    }

    all_records.sort_by(|a, b| {
        let a_ts = OffsetDateTime::parse(&a.record.ts, &Rfc3339).ok();
        let b_ts = OffsetDateTime::parse(&b.record.ts, &Rfc3339).ok();
        match (a_ts, b_ts) {
            (Some(a_ts), Some(b_ts)) => a_ts.cmp(&b_ts),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.record.ts.cmp(&b.record.ts),
        }
    });
    Ok(all_records)
}

pub fn history_path_candidates(xdg_state_home: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(base) = xdg_state_home {
        paths.push(base.join("rt").join("history.jsonl"));
    }
    if let Some(base) = home {
        paths.push(
            base.join(".local")
                .join("state")
                .join("rt")
                .join("history.jsonl"),
        );
        paths.push(base.join(".rt").join("history.jsonl"));
    }
    if paths.is_empty() {
        paths.push(PathBuf::from(".rt").join("history.jsonl"));
    }
    paths
}

fn current_timestamp() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    now.format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00+00:00".to_string())
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn engine_for_runner(runner: Runner) -> HistoryEngine {
    match runner {
        Runner::Makefile | Runner::CargoMake => HistoryEngine::Make,
        Runner::Justfile => HistoryEngine::Just,
        Runner::Taskfile | Runner::Maskfile | Runner::Mise => HistoryEngine::Shell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_record(ts: &str, cmd: &str, exit_code: i32) -> HistoryRecord {
        HistoryRecord {
            v: 1,
            ts: ts.to_string(),
            cmd: cmd.to_string(),
            cwd: "/repo".to_string(),
            exit_code,
            duration_ms: 120,
            engine: Some(HistoryEngine::Make),
            target: Some("build".to_string()),
            file: Some("/repo/Makefile".to_string()),
            hostname: None,
            user: None,
        }
    }

    #[test]
    fn history_path_candidates_include_home_fallback() {
        let paths =
            history_path_candidates(Some(Path::new("/tmp/state")), Some(Path::new("/tmp/home")));
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/state/rt/history.jsonl"),
                PathBuf::from("/tmp/home/.local/state/rt/history.jsonl"),
                PathBuf::from("/tmp/home/.rt/history.jsonl"),
            ]
        );
    }

    #[test]
    fn history_path_candidates_fall_back_without_xdg() {
        let paths = history_path_candidates(None, Some(Path::new("/tmp/home")));
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/tmp/home/.local/state/rt/history.jsonl"),
                PathBuf::from("/tmp/home/.rt/history.jsonl")
            ]
        );
    }

    #[test]
    fn from_input_sets_required_and_optional_fields() {
        let cwd = PathBuf::from("/repo");
        let record = HistoryRecord::from_input(RecordInput {
            runner: Runner::Justfile,
            command: "just test",
            task: "test",
            cwd: &cwd,
            exit_code: 7,
            duration_ms: 34,
            runner_file: Some(Path::new("/repo/justfile")),
        });
        assert_eq!(record.v, 1);
        assert_eq!(record.cmd, "just test");
        assert_eq!(record.cwd, "/repo");
        assert_eq!(record.exit_code, 7);
        assert_eq!(record.duration_ms, 34);
        assert_eq!(record.engine, Some(HistoryEngine::Just));
        assert_eq!(record.target.as_deref(), Some("test"));
        assert_eq!(record.file.as_deref(), Some("/repo/justfile"));
        assert!(record.ts.contains('T'));
    }

    #[test]
    fn from_shell_input_sets_shell_engine_without_target_or_file() {
        let cwd = PathBuf::from("/repo");
        let record = HistoryRecord::from_shell_input(ShellRecordInput {
            command: "make build",
            cwd: &cwd,
            exit_code: 3,
            duration_ms: 11,
        });
        assert_eq!(record.engine, Some(HistoryEngine::Shell));
        assert!(record.target.is_none());
        assert!(record.file.is_none());
        assert_eq!(record.cmd, "make build");
        assert_eq!(record.cwd, "/repo");
        assert_eq!(record.exit_code, 3);
    }

    #[test]
    fn store_append_creates_directories_and_can_read_back() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join("a/b/c/history.jsonl");
        let store = HistoryStore::new(history_path.clone());
        let record = sample_record("2026-02-21T12:34:56+09:00", "make build", 0);

        store.append(&record).unwrap();
        assert!(history_path.exists());

        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record, record);
    }

    #[test]
    fn store_read_all_ignores_invalid_json_lines() {
        let dir = tempdir().unwrap();
        let history_path = dir.path().join("history.jsonl");
        fs::write(
            &history_path,
            concat!(
                "not-json\n",
                "{\"v\":1,\"ts\":\"2026-02-21T12:34:56+09:00\",\"cmd\":\"make build\",\"cwd\":\"/repo\",\"exit\":0,\"duration_ms\":10}\n"
            ),
        )
        .unwrap();

        let store = HistoryStore::new(history_path);
        let records = store.read_all().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record.cmd, "make build");
    }

    #[test]
    fn read_from_paths_merges_and_sorts_by_timestamp() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.jsonl");
        let second = dir.path().join("second.jsonl");
        let store_first = HistoryStore::new(first.clone());
        let store_second = HistoryStore::new(second.clone());
        store_first
            .append(&sample_record("2026-02-21T12:02:00+09:00", "make c", 0))
            .unwrap();
        store_second
            .append(&sample_record("2026-02-21T12:01:00+09:00", "make b", 0))
            .unwrap();
        store_first
            .append(&sample_record("2026-02-21T12:03:00+09:00", "make d", 0))
            .unwrap();

        let records = read_from_paths(vec![first, second]).unwrap();
        let commands: Vec<String> = records
            .into_iter()
            .map(|record| record.record.cmd)
            .collect();
        assert_eq!(
            commands,
            vec![
                "make b".to_string(),
                "make c".to_string(),
                "make d".to_string()
            ]
        );
    }

    #[test]
    fn read_from_paths_ignores_unreadable_path_if_others_work() {
        let dir = tempdir().unwrap();
        let unreadable = dir.path().join("unreadable");
        fs::create_dir_all(&unreadable).unwrap();

        let valid = dir.path().join("history.jsonl");
        let valid_store = HistoryStore::new(valid.clone());
        valid_store
            .append(&sample_record("2026-02-21T12:04:00+09:00", "make e", 0))
            .unwrap();

        let records = read_from_paths(vec![unreadable, valid]).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record.cmd, "make e");
    }

    #[test]
    fn history_path_candidates_fall_back_to_dot_rt_without_home() {
        let paths = history_path_candidates(None, None);
        assert_eq!(paths, vec![PathBuf::from(".rt/history.jsonl")]);
    }
}
