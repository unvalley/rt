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

#[cfg(test)]
mod tests {
    use super::*;

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
}
