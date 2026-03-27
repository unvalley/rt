use crate::tasks::TaskItem;

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let (name, description) = line.split_once(':')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }

            let description = description.trim();
            Some(TaskItem {
                name: name.to_string(),
                description: (!description.is_empty()).then(|| description.to_string()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vite_plus_task_list() {
        let output = "\
  check: echo check root
  app#build: echo build app
";
        let tasks = parse(output);
        assert_eq!(
            tasks,
            vec![
                TaskItem {
                    name: "check".to_string(),
                    description: Some("echo check root".to_string()),
                },
                TaskItem {
                    name: "app#build".to_string(),
                    description: Some("echo build app".to_string()),
                },
            ]
        );
    }

    #[test]
    fn parse_vite_plus_ignores_blank_lines() {
        let output = "\n\n  build: echo build\n";
        let tasks = parse(output);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
    }
}
