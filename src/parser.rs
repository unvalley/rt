use crate::detect::Runner;
use crate::tasks::TaskItem;

mod cargo_make;
mod justfile;
mod makefile;
mod mise;
mod taskfile;

/// Returns parsed tasks from the output of the given runner's list command.
pub fn parse_tasks(runner: Runner, output: &str) -> Vec<TaskItem> {
    match runner {
        Runner::Justfile => justfile::parse(output),
        Runner::Taskfile => taskfile::parse(output),
        Runner::Mise => mise::parse(output),
        Runner::CargoMake => cargo_make::parse(output),
        Runner::Makefile => makefile::parse(output),
    }
}
