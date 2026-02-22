use std::collections::BTreeMap;

use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let makefile_source = read_makefile_source_from_disk();
    parse_with_makefile_source(output, makefile_source.as_deref())
}

fn parse_with_makefile_source(output: &str, makefile_source: Option<&str>) -> Vec<TaskItem> {
    let has_files_section = output
        .lines()
        .any(|line| line.trim_start().starts_with("# Files"));
    let mut in_files = !has_files_section;
    let mut tasks = BTreeMap::new();
    let mut pending_desc: Option<String> = None;

    for line in output.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            pending_desc = None;
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("# Files") {
            in_files = true;
            pending_desc = None;
            continue;
        }
        if trimmed.starts_with("# Finished") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix(".PHONY:") {
            for name in rest.split_whitespace() {
                tasks.entry(name.to_string()).or_insert(None);
            }
            pending_desc = None;
            continue;
        }
        if !in_files {
            pending_desc = None;
            continue;
        }
        if trimmed.starts_with('#') {
            pending_desc = parse_comment_line(trimmed);
            continue;
        }
        if line.starts_with('\t') || line.starts_with(' ') {
            pending_desc = None;
            continue;
        }

        let (target, _) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let name = target.trim();
        if !is_make_target_name(name) {
            pending_desc = None;
            continue;
        }

        let inline_desc = trimmed
            .split_once(':')
            .and_then(|(_, rest)| rest.split_once('#'))
            .map(|(_, comment)| comment.trim())
            .filter(|desc| !desc.is_empty())
            .map(str::to_string);

        let description = inline_desc.or_else(|| pending_desc.take());
        pending_desc = None;

        if description.is_some() {
            tasks.insert(name.to_string(), description);
        } else {
            tasks.entry(name.to_string()).or_insert(None);
        }
    }

    if let Some(source) = makefile_source {
        let descriptions = parse_makefile_descriptions(source);
        for (name, description) in &mut tasks {
            if description.is_none()
                && let Some(source_desc) = descriptions.get(name) {
                    *description = Some(source_desc.clone());
                }
        }
    }

    tasks
        .into_iter()
        .map(|(name, description)| TaskItem { name, description })
        .collect()
}

fn is_make_target_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains('%')
        && !name.contains('$')
        && !name.contains('=')
        && name != "Makefile"
        && name != "makefile"
        && name != "GNUmakefile"
}

fn parse_comment_line(line: &str) -> Option<String> {
    let comment = line.trim_start_matches('#').trim();
    if comment.is_empty() {
        return None;
    }
    if comment.starts_with("Files")
        || comment.starts_with("Finished")
        || comment.starts_with("Not a target")
        || comment.ends_with(':')
    {
        return None;
    }
    Some(comment.to_string())
}

fn read_makefile_source_from_disk() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    for name in ["Makefile", "makefile", "GNUmakefile"] {
        let path = cwd.join(name);
        if path.is_file() {
            return std::fs::read_to_string(path).ok();
        }
    }
    None
}

fn parse_makefile_descriptions(source: &str) -> BTreeMap<String, String> {
    let mut descriptions = BTreeMap::new();
    let mut pending_desc: Option<String> = None;

    for line in source.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            pending_desc = None;
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            pending_desc = parse_comment_line(trimmed);
            continue;
        }
        if line.starts_with('\t') || line.starts_with(' ') {
            pending_desc = None;
            continue;
        }

        let (target, rest) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => {
                pending_desc = None;
                continue;
            }
        };

        if rest.trim_start().starts_with('=') {
            pending_desc = None;
            continue;
        }

        let target_names: Vec<&str> = target
            .split_whitespace()
            .map(str::trim)
            .filter(|name| is_make_target_name(name))
            .collect();

        if target_names.is_empty() {
            pending_desc = None;
            continue;
        }

        let inline_desc = trimmed
            .split_once(':')
            .and_then(|(_, value)| value.split_once('#'))
            .map(|(_, comment)| comment.trim())
            .filter(|desc| !desc.is_empty())
            .map(str::to_string);

        let description = inline_desc.or_else(|| pending_desc.take());
        pending_desc = None;

        if let Some(description) = description {
            for target_name in target_names {
                descriptions.insert(target_name.to_string(), description.clone());
            }
        }
    }

    descriptions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_make_list() {
        let output = "\
all: deps build
.PHONY: all
install:
\t@echo install
%.o: %.c
";
        let tasks = parse(output);
        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["all", "install"]);
    }

    #[test]
    fn parse_make_files_section() {
        let output = "\
# Files
all: deps build
install:
\t@echo install

# Finished Make data base
";
        let tasks = parse(output);
        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["all", "install"]);
    }

    #[test]
    fn parse_make_comment_above_target() {
        let output = "\
# build main
build:
\tcc *.c -o main
";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("build main"));
    }

    #[test]
    fn parse_make_inline_comment() {
        let output = "\
build: # build main
\tcc *.c -o main
";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("build main"));
    }

    #[test]
    fn parse_make_uses_makefile_comments_as_description() {
        let output = "\
# Files
build:
\tcc *.c -o main
test-all: build
\t./test --all

# Finished Make data base
";
        let makefile_source = "\
# build main
build:
\tcc *.c -o main

# test everything
test-all: build
\t./test --all
";
        let tasks = parse_with_makefile_source(output, Some(makefile_source));
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("build main"));
        assert_eq!(tasks[1].name, "test-all");
        assert_eq!(tasks[1].description.as_deref(), Some("test everything"));
    }

    #[test]
    fn parse_makefile_descriptions_ignores_variable_assignment() {
        let source = "\
# should not attach to variable
FOO := bar

# build main
build:
\tcc *.c -o main
";
        let descriptions = parse_makefile_descriptions(source);
        assert_eq!(descriptions.get("FOO"), None);
        assert_eq!(descriptions.get("build"), Some(&"build main".to_string()));
    }
}
