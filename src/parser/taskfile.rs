use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let mut items = Vec::new();
    for line in output.lines() {
        let mut line = line.trim_start();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("task:") || line.starts_with("Available") {
            continue;
        }

        if let Some(stripped) = line.strip_prefix("* ") {
            line = stripped;
        } else if let Some(stripped) = line.strip_prefix("- ") {
            line = stripped;
        }

        let (name, desc) = match line.split_once(':') {
            Some((name, desc)) => (name.trim(), Some(desc.trim())),
            None => (line.trim(), None),
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_task_list() {
        let output = "\
task: Available tasks for this project:
* build: Build the project
* test: Run tests
";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("Build the project"));
    }
}
