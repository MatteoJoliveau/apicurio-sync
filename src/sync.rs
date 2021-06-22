use std::path::Path;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::Error;
use crate::plan::Plan;
use crate::provider::{Provider, PushArtifactMetadata};

pub async fn pull_artifacts(provider: &impl Provider, plan: &Plan, workdir: &Path) -> Result<(), Error> {
    for (path, artifact) in &plan.pull {
        let content = provider.fetch_artifact_by_global_id(artifact.global_id.unwrap()).await?;
        let destination = workdir.join(path);
        tokio::fs::create_dir_all(&destination.parent().unwrap()).await?;
        let mut file = File::create(&destination).await?;
        file.write_all(&content).await?;
    }

    Ok(())
}

pub async fn push_artifacts(provider: &impl Provider, plan: &Plan, workdir: &Path) -> Result<(), Error> {
    for (path, artifact) in &plan.push {
        let source = workdir.join(path);
        let mut file = File::open(source).await?;
        let mut content = Vec::new();
        file.read_to_end(&mut content).await?;
        provider.push_artifact(PushArtifactMetadata {
            group_id: artifact.group.clone().unwrap(),
            artifact_id: artifact.artifact.clone().unwrap(),
            name: artifact.name.clone(),
            description: artifact.description.clone(),
            artifact_type: artifact.artifact_type.clone(),
            labels: artifact.labels.clone(),
            properties: artifact.properties.clone(),
        }, content).await?;
    }

    Ok(())
}
