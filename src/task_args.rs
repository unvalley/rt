use std::path::Path;

use crate::detect::{Detection, Runner};

pub fn required_args_for_task(
    detection: &Detection,
    task: &str,
) -> Result<Vec<String>, std::io::Error> {
    match detection.runner {
        Runner::Justfile => parse_justfile_required_args(&detection.runner_file, task),
        _ => Ok(Vec::new()),
    }
}

fn parse_justfile_required_args(path: &Path, task: &str) -> Result<Vec<String>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;

    for line in content.lines() {
        if let Some(required) = parse_required_from_just_header(line, task) {
            return Ok(required);
        }
    }

    Ok(Vec::new())
}

fn parse_required_from_just_header(line: &str, task: &str) -> Option<Vec<String>> {
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }

    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let header_end = trimmed.find(':')?;
    if trimmed
        .find(":=")
        .is_some_and(|assign_pos| assign_pos <= header_end)
    {
        return None;
    }

    let left = trimmed[..header_end].trim();
    if left.is_empty() {
        return None;
    }

    let mut parts = left.split_whitespace();
    let raw_name = parts.next()?;
    let name = raw_name.trim_start_matches('@');
    if name != task {
        return None;
    }

    let mut required = Vec::new();
    for raw in parts {
        let token = raw.trim_end_matches(',');
        if token.is_empty() || token.contains('=') || token.starts_with('*') {
            continue;
        }

        let clean = token.trim_start_matches(['$', '+', '*']);
        if clean.is_empty() {
            continue;
        }

        required.push(clean.to_string());
    }

    Some(required)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_required_from_just_header_extracts_only_required_args() {
        let header = "test TEST ENV='prod' +FILES *REST: build";
        let required = parse_required_from_just_header(header, "test").unwrap();
        assert_eq!(required, vec!["TEST".to_string(), "FILES".to_string()]);
    }

    #[test]
    fn parse_required_from_just_header_ignores_non_recipe_lines() {
        assert!(parse_required_from_just_header("foo := 'bar'", "foo").is_none());
        assert!(parse_required_from_just_header("  test TEST:", "test").is_none());
        assert!(parse_required_from_just_header("# test TEST:", "test").is_none());
    }

    #[test]
    fn parse_justfile_required_args_reads_matching_recipe() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("justfile");
        std::fs::write(
            &path,
            r#"
build:
  echo build

test TEST ENV='prod':
  echo {{TEST}}
"#,
        )
        .unwrap();

        let args = parse_justfile_required_args(&path, "test").unwrap();
        assert_eq!(args, vec!["TEST".to_string()]);
    }
}
