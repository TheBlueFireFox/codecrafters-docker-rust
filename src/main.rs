use std::path::PathBuf;

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
    command: PathBuf,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    match &args.command {
        Command::Run(com) => run::run(com),
    }
}

mod run {
    use std::{os::unix::fs, path::Path};

    use super::*;

    pub fn run(com: &RunCommand) -> Result<()> {
        // keep the tmpfile around
        let _tmpfile = chroot(&com.command).context("an error while chrooting")?;

        // we are in a new root here
        let filename = com
            .command
            .file_name()
            .context("unable to get the file name")?;

        let mut fname = PathBuf::new();
        fname.push("/");
        fname.push(filename);

        let mut child = std::process::Command::new(&fname)
            .args(&com.args)
            .spawn()
            .with_context(|| format!("Tried to run {:?} with arguments {:?}", fname, com.args))?;

        let es = child.wait()?;

        match es.code() {
            None => Ok(()),
            Some(0) => Ok(()),
            Some(n) => std::process::exit(n),
        }
    }

    fn chroot(prog: &Path) -> Result<tempfile::TempDir> {
        let dir = tempfile::tempdir().context("creating the tmp dir")?;
        let mut tmp_path = PathBuf::from(dir.path());
        tmp_path.push(prog.file_name().context("unable to get file path")?);

        std::fs::copy(prog, &tmp_path).context("copying the program")?;
        tmp_path.pop();

        tmp_path.push("dev");

        std::fs::create_dir(&tmp_path).context("trying to create the null dir")?;

        tmp_path.push("null");
        std::fs::File::create_new(tmp_path).context("creating /dev/null")?;

        fs::chroot(dir.path()).context("chrooting it :)")?;

        std::env::set_current_dir("/").context("not able to set the current dir")?;

        Ok(dir)
    }
}
