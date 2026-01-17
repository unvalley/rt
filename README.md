## rt : runs tasks right

One command to run tasks across task runners.
Inspired by [antfu/ni](https://github.com/antfu/ni).

### What it does

`rt` looks for task runer files below, and runs the appropriate task runner command.

- make: `Makefile`
- just: `justfile` / `Justfile`
- task: `Taskfile.yml` / `Taskfile.yaml` ...
- cargo-make: `Makefile.toml`
- mise: `mise.toml`
- mask: `maskfile.md`

### rt is useful if you

- don’t want to care whether a repo uses make, just, or task
- want to select and run tasks with an interactive UI

### Install

```sh
cargo install rt-cli
```

```sh
cargo binstall rt-cli
```

Planned:

- homebrew
- nix

### `rt`: run tasks selectively

```sh
rt
```

If a task runner is found, rt shows an interactive task selector:

```sh
> rt

? Select task
> build     - build main
  test-all  - test everything
  test      - run a specific test
[↑↓ to move, enter to select, type to filter]
```

### `rt <task>`: run specific task

```sh
rt <task> [-- args...]
```

### Why?

There are many task runners available, and different projects use different ones.
