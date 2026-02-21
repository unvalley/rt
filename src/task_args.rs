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

    let header_end = find_top_level_colon(trimmed)?;
    if trimmed[header_end..].starts_with(":=") {
        return None;
    }

    let left = trimmed[..header_end].trim();
    if left.is_empty() {
        return None;
    }

    let parts = split_top_level_whitespace(left);
    let raw_name = parts.first()?;
    let name = raw_name.trim_start_matches('@');
    if name != task {
        return None;
    }

    let mut required = Vec::new();
    for raw in parts.into_iter().skip(1) {
        let token = raw.trim_end_matches(',');
        if token.is_empty() || token.starts_with('*') || has_top_level_char(token, '=') {
            continue;
        }

        let clean = token.trim_start_matches(['$', '+', '*']);
        if !is_valid_identifier(clean) {
            continue;
        }

        required.push(clean.to_string());
    }

    Some(required)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    Single,
    Double,
    Backtick,
}

#[derive(Debug, Clone, Copy)]
struct ParseState {
    quote: Option<Quote>,
    escaped: bool,
    paren: usize,
    bracket: usize,
    brace: usize,
}

impl ParseState {
    fn top_level(self) -> bool {
        self.quote.is_none() && self.paren == 0 && self.bracket == 0 && self.brace == 0
    }
}

fn find_top_level_colon(input: &str) -> Option<usize> {
    let mut state = ParseState {
        quote: None,
        escaped: false,
        paren: 0,
        bracket: 0,
        brace: 0,
    };

    for (idx, ch) in input.char_indices() {
        if state.top_level() && ch == ':' {
            return Some(idx);
        }
        advance_state(&mut state, ch);
    }

    None
}

fn split_top_level_whitespace(input: &str) -> Vec<&str> {
    let mut state = ParseState {
        quote: None,
        escaped: false,
        paren: 0,
        bracket: 0,
        brace: 0,
    };
    let mut parts = Vec::new();
    let mut start = None;

    for (idx, ch) in input.char_indices() {
        if state.top_level() && ch.is_whitespace() {
            if let Some(part_start) = start.take() {
                parts.push(&input[part_start..idx]);
            }
            continue;
        }

        if start.is_none() {
            start = Some(idx);
        }
        advance_state(&mut state, ch);
    }

    if let Some(part_start) = start {
        parts.push(&input[part_start..]);
    }

    parts
}

fn has_top_level_char(input: &str, target: char) -> bool {
    let mut state = ParseState {
        quote: None,
        escaped: false,
        paren: 0,
        bracket: 0,
        brace: 0,
    };

    for ch in input.chars() {
        if state.top_level() && ch == target {
            return true;
        }
        advance_state(&mut state, ch);
    }

    false
}

fn advance_state(state: &mut ParseState, ch: char) {
    if let Some(quote) = state.quote {
        if state.escaped {
            state.escaped = false;
            return;
        }
        if ch == '\\' {
            state.escaped = true;
            return;
        }
        let closes_quote = matches!(
            (quote, ch),
            (Quote::Single, '\'') | (Quote::Double, '"') | (Quote::Backtick, '`')
        );
        if closes_quote {
            state.quote = None;
        }
        return;
    }

    match ch {
        '\'' => state.quote = Some(Quote::Single),
        '"' => state.quote = Some(Quote::Double),
        '`' => state.quote = Some(Quote::Backtick),
        '(' => state.paren += 1,
        ')' => state.paren = state.paren.saturating_sub(1),
        '[' => state.bracket += 1,
        ']' => state.bracket = state.bracket.saturating_sub(1),
        '{' => state.brace += 1,
        '}' => state.brace = state.brace.saturating_sub(1),
        _ => {}
    }
}

fn is_valid_identifier(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let mut chars = value.chars();
    let first = chars.next().unwrap_or('_');
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch == '-' || ch.is_ascii_alphanumeric())
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
    fn parse_required_from_just_header_handles_colons_in_default_values() {
        let header = "deploy ENV='prod:blue' TARGET: build";
        let required = parse_required_from_just_header(header, "deploy").unwrap();
        assert_eq!(required, vec!["TARGET".to_string()]);
    }

    #[test]
    fn parse_required_from_just_header_handles_spaces_in_default_values() {
        let header = "test MSG='hello world' TARGET: run";
        let required = parse_required_from_just_header(header, "test").unwrap();
        assert_eq!(required, vec!["TARGET".to_string()]);
    }

    #[test]
    fn parse_required_from_just_header_ignores_star_and_includes_plus() {
        let header = "build +FILES *REST TARGET: run";
        let required = parse_required_from_just_header(header, "build").unwrap();
        assert_eq!(required, vec!["FILES".to_string(), "TARGET".to_string()]);
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

    #[test]
    fn parse_justfile_required_args_with_colon_in_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("justfile");
        std::fs::write(
            &path,
            r#"
deploy ENV='prod:blue' TARGET:
  echo "{{ENV}} {{TARGET}}"
"#,
        )
        .unwrap();

        let args = parse_justfile_required_args(&path, "deploy").unwrap();
        assert_eq!(args, vec!["TARGET".to_string()]);
    }
}
