use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::error::Error;
use crate::provider::{ArtifactType, Provider};

#[derive(Debug, Deserialize, Serialize)]
pub struct LockFile {
    pub push: HashMap<PathBuf, PushArtifactRef>,
    pub pull: HashMap<PathBuf, PullArtifactRef>,
    #[serde(skip)]
    path: PathBuf,
}

impl LockFile {
    fn empty(path: PathBuf) -> Self {
        Self {
            push: HashMap::new(),
            pull: HashMap::new(),
            path,
        }
    }

    pub async fn try_load_for_config(config: &Config, provider: &impl Provider) -> Result<Self, Error> {
        let path = &config.path;
        let path = path.with_file_name(path.file_name().unwrap()).with_extension("lock");
        let lock_file = match File::open(&path).await {
            Ok(file) => Some(file),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => None,
                _ => return Err(err.into()),
            }
        };
        let mut lock_file = if let Some(lock_file) = lock_file {
            let mut lock_file: LockFile = serde_json::from_reader(lock_file.into_std().await).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
            lock_file.path = path;
            lock_file
        } else {
            Self::empty(path)
        };
        lock_file.generate(config, provider, false).await?;
        Ok(lock_file)
    }

    pub async fn update(&mut self, config: &Config, provider: &impl Provider) -> Result<(), Error> {
        self.generate(config, provider, true).await
    }

    async fn generate(&mut self, config: &Config, provider: &impl Provider, update: bool) -> Result<(), Error> {
        let push = &config.push;
        if push.is_empty() {
            self.push = HashMap::new();
        }

        for artifact in push {
            if !update && self.push.contains_key(&artifact.path) {
                continue;
            }

            let locked = PushArtifactRef {
                group: artifact.group.clone(),
                artifact: artifact.artifact.clone(),
                artifact_type: artifact.artifact_type.clone(),
            };

            self.push.insert(artifact.path.clone(), locked);
        }

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
                let metadata = provider.fetch_artifact_version_metadata(&artifact.group, &artifact.artifact, version).await?;
                PullArtifactRef {
                    group: metadata.group_id,
                    artifact: metadata.id,
                    global_id: metadata.global_id,
                    version: metadata.version,
                    artifact_type: metadata.artifact_type,
                }
            } else {
                let metadata = provider.fetch_artifact_metadata(&artifact.group, &artifact.artifact).await?;
                PullArtifactRef {
                    group: metadata.group_id,
                    artifact: metadata.id,
                    global_id: metadata.global_id,
                    version: metadata.version,
                    artifact_type: metadata.artifact_type,
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
pub struct PushArtifactRef {
    pub group: String,
    pub artifact: String,
    pub artifact_type: Option<ArtifactType>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PullArtifactRef {
    pub group: String,
    pub artifact: String,
    pub global_id: u64,
    pub artifact_type: ArtifactType,
    pub version: String,
}
