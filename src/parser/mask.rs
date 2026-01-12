use crate::tasks::TaskItem;

#[derive(Debug, serde::Deserialize)]
struct Maskfile {
    #[serde(default)]
    commands: Vec<Command>,
}

#[derive(Debug, serde::Deserialize)]
struct Command {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    script: Option<serde_json::Value>,
    #[serde(default)]
    subcommands: Vec<Command>,
}

pub(super) fn parse(output: &str) -> Vec<TaskItem> {
    let Ok(maskfile) = serde_json::from_str::<Maskfile>(output) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for command in maskfile.commands {
        collect_tasks(&mut items, command, "");
    }
    items
}

fn collect_tasks(items: &mut Vec<TaskItem>, command: Command, prefix: &str) {
    let name = if prefix.is_empty() {
        command.name
    } else {
        format!("{prefix} {}", command.name)
    };

    if command.script.is_some() {
        items.push(TaskItem {
            name: name.clone(),
            description: clean_description(command.description),
        });
    }

    for subcommand in command.subcommands {
        collect_tasks(items, subcommand, &name);
    }
}

fn clean_description(desc: Option<String>) -> Option<String> {
    desc.and_then(|desc| {
        let trimmed = desc.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mask_introspect() {
        let output = r#"
{
  "commands": [
    {
      "name": "build",
      "description": "Build project",
      "script": {"body": ["echo build"]},
      "subcommands": []
    },
    {
      "name": "gen",
      "description": "",
      "subcommands": [
        {
          "name": "types",
          "description": "Generate types",
          "script": "echo types",
          "subcommands": []
        }
      ]
    }
  ]
}
"#;
        let tasks = parse(output);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description.as_deref(), Some("Build project"));
        assert_eq!(tasks[1].name, "gen types");
        assert_eq!(tasks[1].description.as_deref(), Some("Generate types"));
    }

    #[test]
    fn parse_mask_invalid_json() {
        let output = "not json";
        let tasks = parse(output);
        assert!(tasks.is_empty());
    }
}
