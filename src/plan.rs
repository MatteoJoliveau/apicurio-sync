use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::context::Context;
use crate::lockfile::LockFile;
use crate::provider::ArtifactType;

pub struct Plan {
    pub push: HashMap<PathBuf, PushArtifactRef>,
    pub pull: HashMap<PathBuf, PullArtifactRef>,
    pub ctx: Context,
}

impl Plan {
    pub fn new(ctx: Context) -> Self {
        Self {
            push: HashMap::new(),
            pull: HashMap::new(),
            ctx,
        }
    }

    pub fn merge_with_config(mut self, cfg: &Config) -> Self {
        for artifact in &cfg.pull {
            let mut pull_ref = self
                .pull
                .entry(artifact.path.clone())
                .or_insert_with(PullArtifactRef::default);
            pull_ref.group = Some(artifact.group.clone());
            pull_ref.artifact = Some(artifact.artifact.clone());
            pull_ref.version = artifact.version.clone();
        }

        for artifact in &cfg.push {
            let mut push_ref = self
                .push
                .entry(artifact.path.clone())
                .or_insert_with(PushArtifactRef::default);
            push_ref.group = Some(artifact.group.clone());
            push_ref.artifact = Some(artifact.artifact.clone());
            push_ref.artifact_type = artifact.artifact_type.clone();
            push_ref.name = artifact.name.clone();
            push_ref.description = artifact.description.clone();
            push_ref.labels = artifact.labels.clone();
            push_ref.properties = artifact.properties.clone();
        }
        self
    }

    pub fn merge_with_lockfile(mut self, lockfile: &LockFile) -> Self {
        for (path, artifact) in &lockfile.pull {
            let mut pull_ref = self
                .pull
                .entry(path.clone())
                .or_insert_with(PullArtifactRef::default);
            pull_ref.group = Some(artifact.group.clone());
            pull_ref.artifact = Some(artifact.artifact.clone());
            pull_ref.version = Some(artifact.version.clone());
        }
        self
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PushArtifactRef {
    pub group: Option<String>,
    pub artifact: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub labels: Option<Vec<String>>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PullArtifactRef {
    pub group: Option<String>,
    pub artifact: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub version: Option<String>,
}
