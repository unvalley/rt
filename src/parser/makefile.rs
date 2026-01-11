use std::collections::BTreeSet;

use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let has_files_section = output
        .lines()
        .any(|line| line.trim_start().starts_with("# Files"));
    let mut in_files = !has_files_section;
    let mut names = BTreeSet::new();

    for line in output.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("# Files") {
            in_files = true;
            continue;
        }
        if trimmed.starts_with("# Finished") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix(".PHONY:") {
            for name in rest.split_whitespace() {
                names.insert(name.to_string());
            }
            continue;
        }
        if !in_files || trimmed.starts_with('#') || line.starts_with('\t') || line.starts_with(' ')
        {
            continue;
        }

        let (target, _) = match trimmed.split_once(':') {
            Some(parts) => parts,
            None => continue,
        };

        let name = target.trim();
        if !is_make_target_name(name) {
            continue;
        }

        names.insert(name.to_string());
    }

    names
        .into_iter()
        .map(|name| TaskItem {
            name,
            description: None,
        })
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
}
