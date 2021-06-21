use std::path::{PathBuf, Path};

use clap::arg_enum;
use structopt::StructOpt;

use crate::client::Client;
use crate::config::Config;
use crate::error::Error;
use crate::lockfile::LockFile;
use crate::provider::{NoopProvider, Provider};
use url::Url;

mod client;
mod config;
mod error;
mod lockfile;
mod provider;
mod sync;

#[derive(Debug, StructOpt)]
enum Command {
    #[structopt(
    about = "Updates the project lockfile with the registry without updating the artifacts themselves",
    long_about = "Updates the project lockfile with the registry, by fetching the required version (if specified) or the latest version from the API. This operation does not update the artifacts themselves. Rerun `sync` to do so."
    )]
    Update,
    #[structopt(long_about = "Initializes an empty config file")]
    Init,
    #[structopt(
    about = "Synchronizes artifacts with the registry",
    long_about = "Synchronizes artifacts with the registry. Push operations upload artifacts to the registry, while pull operations downloads them into the specified local folder"
    )]
    Sync,
}

arg_enum! {
    #[derive(PartialEq, Debug)]
    enum ApiVersion {
        V2,
    }
}

#[derive(Debug, StructOpt)]
struct Opts {
    #[structopt(
    short = "f",
    long = "config-file",
    default_value = "apicurio-sync.yaml",
    env = "APICURIO_SYNC_CONFIG_FILE",
    help = "The configuration file to use",
    parse(from_os_str),
    global = true)]
    config: PathBuf,
    #[structopt(long = "api-version",
    default_value = "v2",
    help = "The Apicurio API to use",
    case_insensitive = true,
    possible_values = & ApiVersion::variants(),
    global = true)]
    api_version: ApiVersion,
    #[structopt(long = "url", default_value = "http://localhost:8080", env = "APICURIO_SYNC_REGISTRY_URL", global = true)]
    url: Url,
    #[structopt(
    long = "cwd",
    help = "The working directory to use. Every operation will happen inside this directory. Defaults to the current directory.",
    env = "APICURIO_SYNC_WORKDIR",
    parse(from_os_str),
    global = true)]
    cwd: Option<PathBuf>,
    #[structopt(subcommand)]
    cmd: Option<Command>,
}

async fn run() -> Result<(), Error> {
    let opts: Opts = Opts::from_args();
    let workdir = opts.cwd.unwrap_or_else(|| std::env::current_dir().expect("current_dir"));
    let cfg_file = workdir.join(opts.config);
    if let Some(Command::Init) = opts.cmd {
        return init(cfg_file, &NoopProvider).await;
    }

    let config = Config::load_from_file(cfg_file).await?;
    let client = Client::new(opts.url.clone());
    let client_v2 = client.v2();
    let mut lockfile = LockFile::try_load_for_config(&config, &client_v2).await?;
    match opts.cmd.as_ref().unwrap_or(&Command::Sync {}) {
        Command::Update => update(&client_v2, &config, &mut lockfile).await,
        Command::Sync => sync(&client_v2, &lockfile, &workdir).await,
        Command::Init => /* we already run Init */ Ok(()),
    }
}

async fn update(provider: &impl Provider, config: &Config, lockfile: &mut LockFile) -> Result<(), Error> {
    eprintln!("Updating lockfile with remote registry");
    lockfile.update(config, provider).await?;
    eprintln!("Lockfile update completed. Rerun sync to update the artifacts");
    Ok(())
}

async fn init(cfg_file: PathBuf, provider: &impl Provider) -> Result<(), Error> {
    let config = Config::write_empty(cfg_file).await?;
    LockFile::try_load_for_config(&config, provider).await?;
    Ok(())
}

async fn sync(provider: &impl Provider, lockfile: &LockFile, workdir: &Path) -> Result<(), Error> {
    eprintln!("Syncing artifacts with remote registry");
    sync::pull_artifacts(provider, lockfile, workdir).await?;
    sync::push_artifacts(provider, lockfile, workdir).await?;
    eprintln!("Sync completed");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {}", err)
    }
}
