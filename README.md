# rt : run task runner right

`rt` runs the right task runner for the project.
Inspired by [antfu/ni](https://github.com/antfu/ni).

## What it does

`rt` looks at the current directory,
detects the task runner,
and runs it for you.

Supported files:

- `justfile` / `Justfile`
- `Taskfile.yml` / `Taskfile.yaml` ...
- `Makefile.toml`
- `Makefile`


## Usage

```sh
rt <task> [-- args...]
```

## Install

wip
