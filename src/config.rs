use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs::{File, OpenOptions};
use url::Url;
use crate::provider::ArtifactType;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub url: Url,
    #[serde(default)]
    pub push: Vec<PushArtifactRef>,
    #[serde(default)]
    pub pull: Vec<PullArtifactRef>,
    #[serde(skip)]
    pub path: PathBuf,
}

impl Config {
    pub async fn load_from_file(path: PathBuf) -> std::io::Result<Self> {
        let cfg_file = File::open(&path).await?;
        let mut cfg_yaml: Config = serde_yaml::from_reader(cfg_file.into_std().await).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
        cfg_yaml.path = path;
        Ok(cfg_yaml)
    }

    pub async fn write_empty(path: PathBuf) -> std::io::Result<Self> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path).await?;
        let cfg = Config{ path, ..Default::default() };
        let content = serde_yaml::to_vec(&cfg).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
        file.write_all(&content).await?;
        Ok(cfg)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            url: "https://example.com".parse().unwrap(),
            push: Vec::new(),
            pull: Vec::new(),
            path: PathBuf::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PushArtifactRef {
    pub group: String,
    pub artifact: String,
    pub path: PathBuf,
    #[serde(rename = "type")]
    pub artifact_type: Option<ArtifactType>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PullArtifactRef {
    pub group: String,
    pub artifact: String,
    pub version: Option<String>,
    pub path: PathBuf,
}
