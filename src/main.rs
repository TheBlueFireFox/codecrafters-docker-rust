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
    println!("Logs from your program will appear here!");

    match &args.command {
        Command::Run(com) => run(com),
    }
}

fn run(com: &RunCommand) -> Result<()> {
    let output = std::process::Command::new(&com.command)
        .args(&com.args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                com.command, com.args
            )
        })?;

    if output.status.success() {
        let std_out = std::str::from_utf8(&output.stdout)?;
        println!("{}", std_out);
    } else {
        std::process::exit(1);
    }

    Ok(())
}
