## rt : run task runner right

`rt` runs the right task runner for the project.
Inspired by [antfu/ni](https://github.com/antfu/ni).

### What it does

`rt` looks for task runer files below, and runs the appropriate task runner command.

Supported files:

- `justfile` / `Justfile`
- `Taskfile.yml` / `Taskfile.yaml` ...
- `Makefile.toml`
- `Makefile`

### Install

```sh
cargo install rt-cli
```

will be supported soon:

- homebrew
- nix

### `rt`: run tasks selectively

```sh
rt
```

### `rt <task>`: run specific task

```sh
rt <task> [-- args...]
```

### Why?

There are many task runners available, and different projects use different ones.
