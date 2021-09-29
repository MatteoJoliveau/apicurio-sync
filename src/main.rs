#[macro_use]
extern crate lazy_static;

use std::future::Future;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::auth::basic::BasicAuthProvider;
use crate::auth::oidc::OidcProvider;
use crate::auth::AuthProvider;
use structopt::StructOpt;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::client::Client;
use crate::config::Config;
use crate::context::Context;
use crate::error::Error;
use crate::lockfile::LockFile;
use crate::plan::Plan;
use crate::provider::{NoopProvider, Provider};

mod auth;
mod client;
mod config;
mod context;
mod error;
mod lockfile;
mod plan;
mod provider;
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
    #[structopt(about = "Print registry information for debugging purposes")]
    Info,
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
    #[structopt(long_about = "Authenticate with the current registry")]
    Login(LoginCommand),
}

#[derive(Debug, StructOpt)]
enum LoginCommand {
    Oidc {
        #[structopt(long, help = "The OIDC Client ID to use")]
        client_id: String,
        #[structopt(long, help = "The OIDC Client Secret to use")]
        client_secret: Option<String>,
        #[structopt(
            long,
            help = "The OIDC scope to use",
            default_value = "openid profile email offline_access"
        )]
        scope: String,
        #[structopt(
            short,
            long,
            help = "Local network port to use for receiving the authentication info",
            default_value = "9876"
        )]
        port: u16,
        issuer_url: String,
    },
    Basic {
        #[structopt(short, long, help = "Username")]
        username: String,
        #[structopt(
            long,
            help = "Signals that the password will be provided via stdin. If false, no password is set"
        )]
        password_stdin: bool,
    },
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
        global = true
    )]
    config: PathBuf,
    #[structopt(
    long = "context-file",
    default_value = & CONTEXT_FILE,
    env = "APICURIO_SYNC_CONTEXT_FILE",
    help = "The context file to use",
    parse(from_os_str),
    global = true)]
    context: PathBuf,
    #[structopt(
        long = "cwd",
        help = "The working directory to use. Every operation will happen inside this directory. Defaults to the current directory.",
        env = "APICURIO_SYNC_WORKDIR",
        parse(from_os_str),
        global = true
    )]
    cwd: Option<PathBuf>,
    #[structopt(
    short,
    long,
    help = "Whether to print debug logs or not",
    global = true)]
    debug: bool,
    #[structopt(subcommand)]
    cmd: Option<Command>,
}

async fn run() -> Result<(), Error> {
    let opts: Opts = Opts::from_args();
    let debug = opts.debug;
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", format!("apicurio_sync={}", if debug { "debug" } else { "info" }));
    }

    tracing_subscriber::fmt::init();
    let workdir = opts
        .cwd
        .unwrap_or_else(|| std::env::current_dir().expect("current_dir"));
    let cfg_file = workdir.join(opts.config);
    if let Some(Command::Init) = opts.cmd {
        return init(cfg_file, &NoopProvider, &context::Auth::None).await;
    }

    let ctx_path = &opts.context;
    let ctx_fn = |path| async move { Context::try_new(path, None).await };
    if let Some(Command::Context(cmd)) = opts.cmd {
        return context(cmd, ctx_path.as_path(), ctx_fn).await;
    }

    let ctx = ctx_fn(ctx_path).await?;
    let auth = ctx.auth.clone();
    let config = Config::load_from_file(cfg_file).await?;
    let client_v2 = Client::new(ctx.registry_url.clone()).v2();
    let mut lockfile = LockFile::try_load_for_config(&config, &client_v2, &auth).await?;
    let plan = Plan::new(ctx)
        .merge_with_config(&config)
        .merge_with_lockfile(&lockfile);
    match opts.cmd.as_ref().unwrap_or(&Command::Sync {}) {
        Command::Update => update(&client_v2, &config, &mut lockfile, &auth).await,
        Command::Sync => sync(&client_v2, &plan, &workdir, &auth).await,
        Command::Info => info(&client_v2, &auth).await,
        Command::Context(_) =>
        /* We already run Context */
        {
            Ok(())
        }
        Command::Init =>
        /* we already run Init */
        {
            Ok(())
        }
    }
}

async fn update(
    provider: &impl Provider,
    config: &Config,
    lockfile: &mut LockFile,
    auth: &context::Auth,
) -> Result<(), Error> {
    tracing::info!("Updating lockfile with remote registry");
    lockfile.update(config, provider, auth).await?;
    tracing::info!("Lockfile update completed. Rerun sync to update the artifacts");
    Ok(())
}

async fn init(
    cfg_file: PathBuf,
    provider: &impl Provider,
    auth: &context::Auth,
) -> Result<(), Error> {
    let config = Config::write_empty(cfg_file).await?;
    LockFile::try_load_for_config(&config, provider, auth).await?;
    Ok(())
}

async fn sync(
    provider: &impl Provider,
    plan: &Plan,
    workdir: &Path,
    auth: &context::Auth,
) -> Result<(), Error> {
    tracing::info!("Syncing artifacts with remote registry");
    sync::pull_artifacts(provider, plan, workdir, auth).await?;
    sync::push_artifacts(provider, plan, workdir, auth).await?;
    tracing::info!("Sync completed");
    Ok(())
}

async fn context<
    P: AsRef<Path>,
    Fut: Future<Output = Result<Context, Error>>,
    Fun: FnOnce(P) -> Fut,
>(
    cmd: ContextCommand,
    ctx_path: P,
    load_ctx: Fun,
) -> Result<(), Error> {
    match cmd {
        ContextCommand::Current => {
            let ctx = load_ctx(ctx_path).await?;
            tracing::info!("{}", ctx.context_name);
            Ok(())
        }
        ContextCommand::Init => {
            Context::write_empty_file(ctx_path.as_ref()).await?;
            tracing::info!("Initialzed empty context file");
            Ok(())
        }
        ContextCommand::Set {
            context_name,
            url,
            current,
        } => {
            let path = ctx_path.as_ref();
            let mut ctx = Context::from_file(path, Some(context_name.clone()))
                .await?
                .or_else(|| {
                    url.clone()
                        .map(|url| Context::new(context_name.clone(), url))
                })
                .ok_or_else(|| Error::setup("URL is required to create a new context"))?;
            if let Some(url) = url {
                ctx.registry_url = url;
            }
            ctx.write(path, current).await?;
            tracing::info!("Updated context {}", context_name);
            Ok(())
        }
        ContextCommand::Show => {
            let mut file = File::open(ctx_path.as_ref()).await?;
            let mut buf = String::new();
            file.read_to_string(&mut buf).await?;
            println!("{}", buf);
            Ok(())
        }
        ContextCommand::Login(cmd) => login(cmd, ctx_path).await,
    }
}

async fn login<P: AsRef<Path>>(cmd: LoginCommand, ctx_path: P) -> Result<(), Error> {
    let path = ctx_path.as_ref();
    let ctx = Context::from_file(path, None)
        .await?
        .ok_or_else(|| Error::setup("No current context configured!"))?;

    let provider: Box<dyn AuthProvider> = match cmd {
        LoginCommand::Oidc {
            issuer_url,
            client_id,
            client_secret,
            scope,
            port,
        } => Box::new(OidcProvider::new(issuer_url, client_id, client_secret, scope, port).await?),
        LoginCommand::Basic {
            username,
            password_stdin,
        } => {
            let password = if password_stdin {
                let mut pwd = String::new();
                std::io::stdin().lock().read_line(&mut pwd)?;

                Some(pwd.trim_end_matches('\n').to_string())
            } else {
                None
            };
            Box::new(BasicAuthProvider::new(username, password))
        }
    };

    let ctx = provider.login(ctx).await?;
    ctx.write(path, true).await?;
    tracing::info!("Updated context auth information");
    Ok(())
}

async fn info(provider: &impl Provider, auth: &context::Auth) -> Result<(), Error> {
    let info = provider.system_info(auth).await?;
    tracing::info!("{:?}", info);
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::info!("Error: {}", err)
    }
}
