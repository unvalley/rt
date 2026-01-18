use std::collections::BTreeMap;

use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
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
}
