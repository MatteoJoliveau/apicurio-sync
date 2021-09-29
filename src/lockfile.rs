use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::context;
use crate::error::Error;
use crate::provider::Provider;

#[derive(Debug, Deserialize, Serialize)]
pub struct LockFile {
    pub pull: HashMap<PathBuf, PullArtifactRef>,
    #[serde(skip)]
    path: PathBuf,
}

impl LockFile {
    fn empty(path: PathBuf) -> Self {
        Self {
            pull: HashMap::new(),
            path,
        }
    }

    pub async fn try_load_for_config(
        config: &Config,
        provider: &impl Provider,
        auth: &context::Auth,
    ) -> Result<Self, Error> {
        let path = &config.path;
        let path = path
            .with_file_name(path.file_name().unwrap())
            .with_extension("lock");
        let lock_file = match File::open(&path).await {
            Ok(file) => Some(file),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => None,
                _ => return Err(err.into()),
            },
        };
        let mut lock_file = if let Some(lock_file) = lock_file {
            let mut lock_file: LockFile = serde_json::from_reader(lock_file.into_std().await)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
            lock_file.path = path;
            lock_file
        } else {
            Self::empty(path)
        };
        lock_file.generate(config, provider, false, auth).await?;
        Ok(lock_file)
    }

    pub async fn update(&mut self, config: &Config, provider: &impl Provider, auth: &context::Auth) -> Result<(), Error> {
        self.generate(config, provider, true, auth).await
    }

    async fn generate(
        &mut self,
        config: &Config,
        provider: &impl Provider,
        update: bool,
        auth: &context::Auth,
    ) -> Result<(), Error> {
        let pull = &config.pull;
        if pull.is_empty() {
            self.pull = HashMap::new();
        }

        let mut pull_inserted = HashSet::new();
        for artifact in pull {
            pull_inserted.insert(artifact.path.clone());
            if !update && self.pull.contains_key(&artifact.path) {
                continue;
            }

            let locked = if let Some(version) = &artifact.version {
                let metadata = provider
                    .fetch_artifact_version_metadata(
                        &artifact.group,
                        &artifact.artifact,
                        version,
                        auth,
                    )
                    .await?;
                PullArtifactRef {
                    group: metadata.group_id,
                    artifact: metadata.id,
                    version: metadata.version,
                }
            } else {
                let metadata = provider
                    .fetch_artifact_metadata(&artifact.group, &artifact.artifact, auth)
                    .await?;
                PullArtifactRef {
                    group: metadata.group_id,
                    artifact: metadata.id,
                    version: metadata.version,
                }
            };
            self.pull.insert(artifact.path.clone(), locked);
        }

        let keys: HashSet<PathBuf> = self.pull.keys().cloned().collect();
        for key in keys.difference(&pull_inserted) {
            self.pull.remove(key);
        }

        let mut file = File::create(&self.path).await?;
        let content = serde_json::to_vec_pretty(&self).expect("LockFile JSON render");
        file.write_all(&content).await.map_err(Error::from)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PullArtifactRef {
    pub group: String,
    pub artifact: String,
    pub version: String,
}
