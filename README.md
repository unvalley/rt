# ri : run it right

`ri` runs the right task runner for the project.
Inspired by [antfu/ni](https://github.com/antfu/ni).

## What it does

`ri` looks at the current directory,
detects the task runner,
and runs it for you.

Supported files:

- `justfile`
- `Taskfile.yml` / `Taskfile.yaml`
- `Makefile.toml`
- `Makefile`


## Usage

```sh
ri <task> [-- args...]
```

## Install

wip
