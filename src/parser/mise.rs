use crate::tasks::TaskItem;

#[derive(Debug, serde::Deserialize)]
struct MiseTask {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let Ok(tasks) = serde_json::from_str::<Vec<MiseTask>>(output) else {
        return Vec::new();
    };

    tasks
        .into_iter()
        .map(|task| TaskItem {
            name: task.name,
            description: task.description.and_then(|desc| {
                let desc = desc.trim();
                if desc.is_empty() {
                    None
                } else {
                    Some(desc.to_string())
                }
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mise_list() {
        let output = r#"
[
  {"name": "gen-bindings", "description": "Generates TS types"},
  {"name": "gen-schema", "description": ""}
]
"#;
        let tasks = parse(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "gen-bindings");
        assert_eq!(tasks[0].description.as_deref(), Some("Generates TS types"));
        assert_eq!(tasks[1].name, "gen-schema");
        assert_eq!(tasks[1].description, None);
    }
}
