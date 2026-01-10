use bpaf::Bpaf;

#[derive(Debug, Clone)]
pub struct Cli {
    pub task: Option<String>,
    pub passthrough: Vec<String>,
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options)]
struct Args {
    #[bpaf(positional("task"))]
    task: Option<String>,
    #[bpaf(positional("args"), many)]
    rest: Vec<String>,
}

pub fn parse() -> Cli {
    let raw = args().run();
    let passthrough = match raw.rest.split_first() {
        Some((first, rest)) if first == "--" => rest.to_vec(),
        Some((_first, _rest)) => raw.rest,
        None => Vec::new(),
    };

    Cli {
        task: raw.task,
        passthrough,
    }
}
