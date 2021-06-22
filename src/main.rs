#[macro_use]
extern crate lazy_static;

use std::path::{PathBuf, Path};

use clap::arg_enum;
use structopt::StructOpt;

use crate::client::Client;
use crate::config::Config;
use crate::error::Error;
use crate::lockfile::LockFile;
use crate::provider::{NoopProvider, Provider};
use url::Url;
use crate::context::Context;
use crate::plan::Plan;
use std::future::Future;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

mod client;
mod config;
mod context;
mod error;
mod lockfile;
mod provider;
mod plan;
mod sync;

lazy_static! {
    static ref CONFIG_DIR: String = {
        let dir = dirs::config_dir().expect("dirs::config_dir");
        format!("{}/{}", dir.to_str().unwrap(), env!("CARGO_BIN_NAME"))
    };

    static ref CONTEXT_FILE: String = format!("{}/context.json", CONFIG_DIR.as_str());
}

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
    #[structopt(
    about = "Work with context",
    long_about = "Manipulate the local CLI context. The context is used to configure registries and their authentication credentials"
    )]
    Context(ContextCommand),
}

#[derive(Debug, StructOpt)]
enum ContextCommand {
    #[structopt(long_about = "Print current context")]
    Current,
    #[structopt(long_about = "Init context file")]
    Init,
    #[structopt(long_about = "Set context properties")]
    Set {
        #[structopt(short, long, help = "The registry URL to set")]
        url: Option<Url>,
        #[structopt(short, long, help = "Set this context as current")]
        current: bool,
        context_name: String,
    },
    #[structopt(long_about = "Print all context configurations")]
    Show,
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
    #[structopt(
    long = "context-file",
    default_value = &CONTEXT_FILE,
    env = "APICURIO_SYNC_CONTEXT_FILE",
    help = "The context file to use",
    parse(from_os_str),
    global = true)]
    context: PathBuf,
    #[structopt(long = "api-version",
    default_value = "v2",
    help = "The Apicurio API to use",
    case_insensitive = true,
    possible_values = & ApiVersion::variants(),
    global = true)]
    api_version: ApiVersion,
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

    let ctx_path = &opts.context;
    let ctx_fn = |path| async move { Context::try_new(path, None).await };
    if let Some(Command::Context(cmd)) = opts.cmd {
        return context(cmd, ctx_path.as_path(), ctx_fn).await;
    }

    let ctx = ctx_fn(ctx_path).await?;
    let config = Config::load_from_file(cfg_file).await?;
    let client = Client::new(ctx.registry_url.clone());
    let client_v2 = client.v2();
    let mut lockfile = LockFile::try_load_for_config(&config, &client_v2).await?;
    let plan = Plan::new(ctx)
        .merge_with_config(&config)
        .merge_with_lockfile(&lockfile);
    match opts.cmd.as_ref().unwrap_or(&Command::Sync {}) {
        Command::Update => update(&client_v2, &config, &mut lockfile).await,
        Command::Sync => sync(&client_v2, &plan, &workdir).await,
        Command::Context(_) => /* We already run Context */ Ok(()),
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

async fn sync(provider: &impl Provider, plan: &Plan, workdir: &Path) -> Result<(), Error> {
    eprintln!("Syncing artifacts with remote registry");
    sync::pull_artifacts(provider, plan, workdir).await?;
    sync::push_artifacts(provider, plan, workdir).await?;
    eprintln!("Sync completed");
    Ok(())
}

async fn context<P: AsRef<Path>, Fut: Future<Output=Result<Context, Error>>, Fun: FnOnce(P) -> Fut>(cmd: ContextCommand, ctx_path: P, load_ctx: Fun) -> Result<(), Error> {
    match cmd {
        ContextCommand::Current => {
            let ctx = load_ctx(ctx_path).await?;
            eprintln!("{}", ctx.context_name);
            Ok(())
        }
        ContextCommand::Init => {
            Context::write_empty_file(ctx_path.as_ref()).await?;
            eprintln!("Initialzed empty context file");
            Ok(())
        },
        ContextCommand::Set { context_name, url, current } => {
            let path = ctx_path.as_ref();
            let mut ctx = Context::try_new(path, Some(context_name.clone())).await?;
            if let Some(url) = url {
                ctx.registry_url = url;
            }
            ctx.write(path, current).await?;
            eprintln!("Updated context {}", context_name);
            Ok(())
        },
        ContextCommand::Show => {
            let mut file = File::open(ctx_path.as_ref()).await?;
            let mut buf = String::new();
            file.read_to_string(&mut buf).await?;
            println!("{}", buf);
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {}", err)
    }
}
