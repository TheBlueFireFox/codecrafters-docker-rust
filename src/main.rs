use anyhow::{Context, Result};
use clap::Parser;

#[derive(clap::Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug, Clone)]
#[command(version, about, long_about = None)]
enum Command {
    #[command(about = "the run command")]
    Run(RunCommand),
}

#[derive(clap::Args, Debug, Clone)]
struct RunCommand {
    image: String,
    command: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    match &args.command {
        Command::Run(com) => run(com),
    }
}

fn run(com: &RunCommand) -> Result<()> {
    let mut child = std::process::Command::new(&com.command)
        .args(&com.args)
        .spawn()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                com.command, com.args
            )
        })?;

    let es = child.wait()?;

    match es.code() {
        None => Ok(()),
        Some(0) => Ok(()),
        Some(n) => std::process::exit(n),
    }
}
