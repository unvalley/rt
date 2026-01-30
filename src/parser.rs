use crate::detect::Runner;
use std::path::{Path, PathBuf};

use crate::tasks::TaskItem;

mod cargo_make;
mod justfile;
mod makefile;
mod mask;
mod mise;
mod taskfile;

/// Returns parsed tasks from the output of the given runner's list command.
pub fn parse_tasks(runner: Runner, output: &str) -> Vec<TaskItem> {
    match runner {
        Runner::Justfile => justfile::parse(output),
        Runner::Taskfile => taskfile::parse(output),
        Runner::Maskfile => mask::parse(output),
        Runner::Mise => mise::parse(output),
        Runner::CargoMake => cargo_make::parse(output),
        Runner::Makefile => makefile::parse(output),
    }
}

pub fn find_justfile(dir: &Path) -> Option<PathBuf> {
    justfile::find_justfile(dir)
}

pub fn parse_justfile_with_imports(path: &Path) -> Result<Vec<TaskItem>, std::io::Error> {
    justfile::parse_with_imports(path)
}
