use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Available") || line.starts_with("Recipes") {
            continue;
        }

        let (left, desc) = match line.split_once('#') {
            Some((left, desc)) => (left.trim(), Some(desc.trim())),
            None => (line, None),
        };

        let name = left.split_whitespace().next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }

        let description = desc.filter(|d| !d.is_empty()).map(|d| d.to_string());
        items.push(TaskItem {
            name: name.to_string(),
            description,
        });
    }
    items
}

pub(super) fn find_justfile(dir: &Path) -> Option<PathBuf> {
    let candidates = ["Justfile", "justfile"];
    for name in candidates {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub(super) fn parse_with_imports(path: &Path) -> Result<Vec<TaskItem>, std::io::Error> {
    let mut state = ParseState::new();
    let mut stack = Vec::new();
    parse_file(path, 0, &mut state, &mut stack)?;
    let mut entries: Vec<TaskEntry> = state.items.into_values().collect();
    entries.sort_by_key(|entry| entry.order);
    Ok(entries.into_iter().map(|entry| entry.item).collect())
}

#[derive(Debug)]
struct ImportSpec {
    raw: String,
    optional: bool,
}

#[derive(Debug)]
struct ParsedRecipe {
    name: String,
    description: Option<String>,
}

#[derive(Debug)]
struct ParsedFile {
    imports: Vec<ImportSpec>,
    recipes: Vec<ParsedRecipe>,
}

#[derive(Debug)]
struct TaskEntry {
    item: TaskItem,
    depth: usize,
    order: usize,
}

struct ParseState {
    order: usize,
    items: HashMap<String, TaskEntry>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            order: 0,
            items: HashMap::new(),
        }
    }

    fn insert(&mut self, item: TaskItem, depth: usize) {
        let order = self.order;
        self.order = self.order.saturating_add(1);
        let should_replace = match self.items.get(&item.name) {
            None => true,
            Some(existing) => depth < existing.depth || depth == existing.depth,
        };
        if should_replace {
            self.items.insert(
                item.name.clone(),
                TaskEntry {
                    item,
                    depth,
                    order,
                },
            );
        }
    }
}

fn parse_file(
    path: &Path,
    depth: usize,
    state: &mut ParseState,
    stack: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    let normalized = normalize_path(path);
    if stack.iter().any(|p| *p == normalized) {
        return Ok(());
    }

    let contents = std::fs::read_to_string(path)?;
    let parsed = parse_justfile_contents(&contents);

    stack.push(normalized);
    for import in parsed.imports.iter().rev() {
        let resolved = resolve_import_path(path, &import.raw);
        if !resolved.exists() {
            if import.optional {
                continue;
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("import not found: {}", resolved.display()),
            ));
        }
        parse_file(&resolved, depth.saturating_add(1), state, stack)?;
    }
    stack.pop();

    for recipe in parsed.recipes {
        state.insert(
            TaskItem {
                name: recipe.name,
                description: recipe.description,
            },
            depth,
        );
    }

    Ok(())
}

fn parse_justfile_contents(contents: &str) -> ParsedFile {
    let mut imports = Vec::new();
    let mut recipes = Vec::new();
    let mut pending_desc: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            pending_desc = None;
            continue;
        }

        let is_indented = line.starts_with(|c: char| c.is_whitespace());
        let trimmed = line.trim_start();

        if is_indented {
            pending_desc = None;
            continue;
        }

        if let Some(import) = parse_import_line(trimmed) {
            imports.push(import);
            pending_desc = None;
            continue;
        }

        if trimmed.starts_with('#') {
            pending_desc = parse_comment_line(trimmed);
            continue;
        }

        if let Some(recipe) = parse_recipe_line(trimmed, pending_desc.take()) {
            recipes.push(recipe);
            continue;
        }

        pending_desc = None;
    }

    ParsedFile { imports, recipes }
}

fn parse_import_line(line: &str) -> Option<ImportSpec> {
    let (optional, rest) = if let Some(rest) = line.strip_prefix("import?") {
        (true, rest)
    } else if let Some(rest) = line.strip_prefix("import") {
        (false, rest)
    } else {
        return None;
    };

    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let end = rest[1..].find(quote)?;
    let path = &rest[1..1 + end];
    if path.is_empty() {
        return None;
    }

    Some(ImportSpec {
        raw: path.to_string(),
        optional,
    })
}

fn parse_recipe_line(line: &str, pending_desc: Option<String>) -> Option<ParsedRecipe> {
    let (left, inline_desc) = match line.split_once('#') {
        Some((left, desc)) => (left.trim_end(), Some(desc.trim())),
        None => (line, None),
    };

    if left.contains(":=") {
        return None;
    }

    let (before_colon, _) = left.split_once(':')?;
    let name = before_colon.split_whitespace().next()?.trim();
    if name.is_empty() {
        return None;
    }

    let description = inline_desc
        .filter(|desc| !desc.is_empty())
        .map(str::to_string)
        .or(pending_desc);

    Some(ParsedRecipe {
        name: name.to_string(),
        description,
    })
}

fn parse_comment_line(line: &str) -> Option<String> {
    let comment = line.trim_start_matches('#').trim();
    if comment.is_empty() {
        None
    } else {
        Some(comment.to_string())
    }
}

fn resolve_import_path(base: &Path, raw: &str) -> PathBuf {
    let expanded = if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(stripped)
        } else {
            PathBuf::from(raw)
        }
    } else {
        PathBuf::from(raw)
    };

    if expanded.is_absolute() {
        expanded
    } else {
        base.parent().unwrap_or(base).join(expanded)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_just_list() {
        let output = "\
Available recipes:
    build  # build project
    test
";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("build project"));
        assert_eq!(tasks[1].name, "test");
        assert_eq!(tasks[1].description, None);
    }

    #[test]
    fn parse_justfile_imports_recursively() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let nested = root.join("nested.just");
        std::fs::write(
            &nested,
            "\
import 'common.just'

# from nested
shared:
    @echo nested

# from nested recipe
nested:
    @echo nested
",
        )
        .unwrap();

        let common = root.join("common.just");
        std::fs::write(
            &common,
            "\
# from common
shared:
    @echo common
",
        )
        .unwrap();

        let other = root.join("other.just");
        std::fs::write(
            &other,
            "\
# from other
shared:
    @echo other
",
        )
        .unwrap();

        let justfile = root.join("justfile");
        std::fs::write(
            &justfile,
            "\
import 'nested.just'
import 'other.just'
import? 'missing.just'

# from root
root:
    @echo root
",
        )
        .unwrap();

        let tasks = parse_with_imports(&justfile).unwrap();
        let shared = tasks.iter().find(|t| t.name == "shared").unwrap();
        let nested_task = tasks.iter().find(|t| t.name == "nested").unwrap();
        let root_task = tasks.iter().find(|t| t.name == "root").unwrap();

        assert_eq!(shared.description.as_deref(), Some("from nested"));
        assert_eq!(nested_task.description.as_deref(), Some("from nested recipe"));
        assert_eq!(root_task.description.as_deref(), Some("from root"));
    }
}
