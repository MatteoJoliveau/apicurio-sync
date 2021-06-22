use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use url::Url;

use crate::error::Error;

const CONTEXT_NAME_ENVAR: &str = "APICURIO_SYNC_CONTEXT_NAME";
const REGISTRY_URL_ENVAR: &str = "APICURIO_SYNC_REGISTRY_URL";

#[derive(Debug, Clone)]
pub struct Context {
    pub context_name: String,
    pub registry_url: Url,
}

impl Context {
    pub async fn try_new(file: &Path) -> Result<Self, Error> {
        let file_ctx = Self::from_file(file).await?;
        let env_ctx = Self::from_env().await?;
        Self::merge(file_ctx, env_ctx).ok_or_else(|| Error::setup("Failed to read context from either file or env"))
    }

    pub async fn from_file(file: &Path) -> Result<Option<Self>, Error> {
        let file = match File::open(file).await {
            Ok(file) => file,
            Err(err) => return match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err.into()),
            }
        };

        let content: ContextFile = serde_json::from_reader(file.into_std().await)?;
        if let Some(current_context) = content.current_context.as_ref() {
            let RegistryContext { url, .. } = content.contexts.get(current_context).ok_or_else(|| Error::setup(format!("No context found for name '{}'", current_context)))?;
            Ok(Some(Context::new(current_context.clone(), url.clone())))
        } else {
            Ok(None)
        }
    }

    pub async fn from_env() -> Result<Option<Self>, Error> {
        let url = std::env::var(REGISTRY_URL_ENVAR).ok();
        if let Some(url) = url {
            let name = std::env::var(CONTEXT_NAME_ENVAR).ok().unwrap_or(url.clone());
            Ok(Some(Context::new(name, url.parse()?)))
        } else {
            Ok(None)
        }
    }

    pub fn new(context_name: String, registry_url: Url) -> Self {
        Self {
            context_name,
            registry_url,
        }
    }

    fn merge(this: Option<Self>, other: Option<Self>) -> Option<Self> {
        if let Some((mut this, other)) = this.zip(other) {
            this.registry_url = other.registry_url;
            Some(this)
        } else {
            None
        }
    }

    pub async fn write_empty_file(path: &Path) -> Result<(), Error> {
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        let empty_file = ContextFile::default();
        let file = OpenOptions::new().write(true).create_new(true).open(path).await?;
        serde_json::to_writer(file.into_std().await, &empty_file).map_err(Into::into)
    }
}

#[derive(Default, Deserialize, Serialize)]
struct ContextFile {
    current_context: Option<String>,
    contexts: HashMap<String, RegistryContext>,
}

#[derive(Deserialize, Serialize)]
struct RegistryContext {
    url: Url,
}

mod auth {}
