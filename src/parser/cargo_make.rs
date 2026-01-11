use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.ends_with(':') || line.starts_with("Available") || line.starts_with("Tasks") {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }

        let description = parts
            .next()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(|d| d.to_string());

        items.push(TaskItem {
            name: name.to_string(),
            description,
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cargo_make_list() {
        let output = "\
Tasks:
build        Build the project
test         Run tests
";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("Build the project"));
    }
}
