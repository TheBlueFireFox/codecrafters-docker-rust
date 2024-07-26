#![allow(dead_code)]

use std::path::PathBuf;

use anyhow::Result;
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
        Command::Run(com) => run::run(com).await,
    }
}

mod run {
    use super::RunCommand;

    use anyhow::{Context, Result};
    use flate2::bufread::GzDecoder;
    use futures::StreamExt;
    use reqwest::{header, Client};
    use std::os::unix::fs;
    use std::path::PathBuf;

    pub async fn run(com: &RunCommand) -> Result<()> {
        // keep the tmpfile around
        let dir = tempfile::tempdir().context("creating the tmp dir")?;
        eprint!("{com:?}");

        let (lib, tag) = com.image.split_once(':').unwrap_or((&com.image, "latest"));

        let token = Token::request(lib).await?;
        let manifest = get_manifest(lib, tag, &token).await?;

        for layer in manifest.layers {
            handle_layer(lib, &layer.digest, &token, &dir).await?;
        }

        chroot(&dir).context("an error while chrooting")?;

        let mut fname = PathBuf::new();
        fname.push("/");
        fname.push(&com.command);

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

    fn chroot(dir: &tempfile::TempDir) -> Result<()> {
        let mut tmp_path = PathBuf::from(dir.path());
        tmp_path.push("dev");

        if !tmp_path.exists() {
            std::fs::create_dir(&tmp_path).context("trying to create the null dir")?;
        }

        tmp_path.push("null");
        if !tmp_path.exists() {
            std::fs::File::create_new(tmp_path).context("creating /dev/null")?;
        }

        fs::chroot(dir.path()).context("chrooting it :)")?;

        std::env::set_current_dir("/").context("not able to set the current dir")?;

        unsafe {
            libc::unshare(libc::CLONE_NEWPID);
        }

        Ok(())
    }

    #[derive(Debug, Clone, serde::Deserialize)]
    struct Token {
        token: String,
    }

    impl Token {
        async fn request(lib: &str) -> Result<Token> {
            let url = "https://auth.docker.io/token";

            let req = Client::new().get(url).query(&[
                ("service", "registry.docker.io"),
                ("scope", &format!("repository:library/{lib}:pull")),
            ]);

            let body = req.send().await?.json().await?;

            Ok(body)
        }
    }

    #[derive(serde::Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct Manifest {
        media_type: String,
        layers: Vec<Layer>,
    }

    #[derive(serde::Deserialize, Debug)]
    struct Layer {
        digest: String,
    }

    async fn get_manifest(lib: &str, tag: &str, token: &Token) -> Result<Manifest> {
        const ACCEPT: &str = "application/vnd.docker.distribution.manifest.v2+json";
        let url = format!("https://registry.hub.docker.com/v2/library/{lib}/manifests/{tag}");
        let req = Client::new()
            .get(url)
            .bearer_auth(&token.token)
            .header(header::ACCEPT, ACCEPT);

        let res = req.send().await?;

        let val: Manifest = res.json().await?;

        assert_eq!(ACCEPT, val.media_type);

        Ok(val)
    }

    async fn handle_layer(
        lib: &str,
        digest: &str,
        token: &Token,
        dir: &tempfile::TempDir,
    ) -> Result<()> {
        let download_dir = tempfile::tempdir().context("creating the tmp download dir")?;

        let mut layer = download_dir.path().to_path_buf();
        layer.push(digest);

        let layer = load_layer(lib, digest, token).await?;

        extract_layer(&layer, dir).context("unable to extract layer")?;

        Ok(())
    }

    async fn load_layer(lib: &str, digest: &str, token: &Token) -> Result<Vec<u8>> {
        let url = format!(
            "https://registry.hub.docker.com/v2/library/{}/blobs/{}",
            lib, digest
        );
        let mut req = Client::new()
            .get(url)
            .bearer_auth(&token.token)
            .send()
            .await?
            .bytes_stream();

        let mut buf = Vec::new();

        while let Some(item) = req.next().await {
            let item = item?;
            buf.extend(item);
        }

        Ok(buf)
    }

    fn extract_layer(file: &[u8], dir: &tempfile::TempDir) -> Result<()> {
        let decoder = GzDecoder::new(file);
        tar::Archive::new(decoder)
            .unpack(dir)
            .context("unable to extract tar archive")?;
        Ok(())
    }
}
