`rt` is a CLI to run tasks interactively across different task runners.
Inspired by [antfu/ni](https://github.com/antfu/ni).

![demo](https://github.com/user-attachments/assets/6e703525-3f52-4303-b679-ee1abde375db)

### What it does

`rt` looks for files below, and provide a way to execute them selectively

- make: `Makefile`
- just: `justfile` / `Justfile`
- task: `Taskfile.yml` / `Taskfile.yaml` ...
- cargo-make: `Makefile.toml`
- mise: `mise.toml`
- mask: `maskfile.md`

### rt is useful if you

- don’t want to care whether a repo uses make, just, and others
- want to select and run tasks with an interactive UI

### Install

```sh
brew install unvalley/tap/rt
```

```sh
cargo install rt-cli
```

```sh
cargo binstall rt-cli
```

Planned: nix, homebrew(core, after requirements met), others

### `rt`: run tasks selectively

```sh
rt
```

```sh
rt --args
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

After selecting a task, rt prompts required parameters when defined (for example, in `justfile` recipes).
Add `--args` if you also want to enter optional arguments interactively.

### `rt <task>`: run specific task

```sh
rt [--args] <task> [-- args...]
```

Executed commands are appended to shell history as a best effort.
`HISTFILE` is used when available, and `fish` is supported via its default history file path.

### Why?

There are many task runners available, and different projects use different ones.
And, I don't like shell script.
