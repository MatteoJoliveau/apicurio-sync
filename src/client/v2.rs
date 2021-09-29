use std::collections::HashMap;
use std::sync::Arc;

use crate::auth::AuthProvider;
use async_trait::async_trait;
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::Error;
use crate::{context, provider};
use crate::context::Auth;
use crate::provider::{ArtifactType, Provider, PushArtifactMetadata};

/// Client for Apicurio Registry API v2
/// https://www.apicur.io/registry/docs/apicurio-registry/2.0.1.Final/assets-attachments/registry-rest-api.htm

pub struct ClientV2 {
    base_url: Url,
    client: reqwest::Client,
}

impl ClientV2 {
    pub(super) fn new(base_url: Url, client: reqwest::Client) -> Self {
        Self {
            base_url: base_url.join("apis/registry/v2/").unwrap(),
            client,
        }
    }
}

#[async_trait]
impl Provider for ClientV2 {
    async fn system_info(&self, auth: &context::Auth) -> Result<provider::SystemInfo, Error> {
        let req = self
            .client
            .get(self.base_url.join("system/info").unwrap());
        let req = with_auth(req, auth);

        let res: reqwest::Result<SystemInfo> = req
            .send()
            .await?
            .error_for_status()?
            .json()
            .await;
        res.map(Into::into).map_err(Into::into)
    }

    async fn fetch_artifact_metadata(
        &self,
        group_id: &str,
        artifact_id: &str,
        auth: &context::Auth,
    ) -> Result<provider::ArtifactMetadata, Error> {
        let req = self
            .client
            .get(
                self.base_url
                    .join(&format!(
                        "groups/{}/artifacts/{}/meta",
                        group_id, artifact_id
                    ))
                    .unwrap(),
            );
        let req = with_auth(req, auth);

        let res: reqwest::Result<ArtifactMetadata> = req
            .send()
            .await?
            .error_for_status()?
            .json()
            .await;
        res.map(Into::into).map_err(Into::into)
    }

    async fn fetch_artifact_version_metadata(
        &self,
        group_id: &str,
        artifact_id: &str,
        version: &str,
        auth: &context::Auth,
    ) -> Result<provider::ArtifactVersionMetadata, Error> {
        let req = self
            .client
            .get(
                self.base_url
                    .join(&format!(
                        "groups/{}/artifacts/{}/versions/{}/meta",
                        group_id, artifact_id, version
                    ))
                    .unwrap(),
            );
        let req = with_auth(req, auth);

        let res: reqwest::Result<ArtifactVersionMetadata> = req
            .send()
            .await?
            .error_for_status()?
            .json()
            .await;
        res.map(Into::into).map_err(Into::into)
    }

    async fn fetch_artifact_version(
        &self,
        group_id: &str,
        artifact_id: &str,
        version: &str,
        auth: &context::Auth,
    ) -> Result<Vec<u8>, Error> {
        let req = self
            .client
            .get(
                self.base_url
                    .join(&format!(
                        "groups/{}/artifacts/{}/versions/{}",
                        group_id, artifact_id, version
                    ))
                    .unwrap(),
            );
        let req = with_auth(req, auth);

        let body = req
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(body.to_vec())
    }

    async fn push_artifact(
        &self,
        metadata: PushArtifactMetadata,
        content: Vec<u8>,
        auth: &context::Auth,
    ) -> Result<(), Error> {
        let req = self.client.post(
            self.base_url
                .join(&format!("groups/{}/artifacts", metadata.group_id))
                .unwrap(),
        );
        let req = with_auth(req, auth);

        let req = if let Some(typ) = metadata.artifact_type {
            req.header("X-Registry-ArtifactType", typ.to_string())
        } else {
            req
        };

        req
            .header("X-Registry-ArtifactId", &metadata.artifact_id)
            .query(&[("ifExists", "RETURN_OR_UPDATE")])
            .body(content)
            .send()
            .await?
            .error_for_status()?;

        self.client
            .put(
                self.base_url
                    .join(&format!(
                        "groups/{}/artifacts/{}/meta",
                        metadata.group_id, metadata.artifact_id
                    ))
                    .unwrap(),
            )
            .json(&UpdateArtifactMetadataBody {
                name: metadata.name,
                description: metadata.description,
                labels: metadata.labels,
                properties: metadata.properties,
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

fn with_auth(req: RequestBuilder, auth: &context::Auth) -> RequestBuilder {
    match auth {
        Auth::Oidc { access_token, .. } => req.bearer_auth(access_token),
        Auth::None => req,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    name: String,
    description: String,
    version: String,
    built_on: String,
}

#[allow(clippy::from_over_into)]
impl Into<provider::SystemInfo> for SystemInfo {
    fn into(self) -> provider::SystemInfo {
        provider::SystemInfo {
            name: self.name,
            description: self.description,
            version: self.version,
            built_on: self.built_on,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactMetadata {
    group_id: String,
    id: String,
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    artifact_type: ArtifactType,
    version: String,
    created_by: String,
    created_on: String,
    modified_by: String,
    modified_on: String,
    global_id: u64,
    content_id: u64,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    properties: HashMap<String, String>,
}

#[allow(clippy::from_over_into)]
impl Into<provider::ArtifactMetadata> for ArtifactMetadata {
    fn into(self) -> provider::ArtifactMetadata {
        provider::ArtifactMetadata {
            group_id: self.group_id,
            id: self.id,
            name: self.name,
            description: self.description,
            artifact_type: self.artifact_type,
            version: self.version,
            created_by: self.created_by,
            created_on: self.created_on,
            modified_by: self.modified_by,
            modified_on: self.modified_on,
            global_id: self.global_id,
            content_id: self.content_id,
            labels: self.labels,
            properties: self.properties,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactVersionMetadata {
    group_id: String,
    id: String,
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    artifact_type: ArtifactType,
    version: String,
    created_by: String,
    created_on: String,
    global_id: u64,
    content_id: u64,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    properties: HashMap<String, String>,
}

#[allow(clippy::from_over_into)]
impl Into<provider::ArtifactVersionMetadata> for ArtifactVersionMetadata {
    fn into(self) -> provider::ArtifactVersionMetadata {
        provider::ArtifactVersionMetadata {
            group_id: self.group_id,
            id: self.id,
            name: self.name,
            description: self.description,
            artifact_type: self.artifact_type,
            version: self.version,
            created_by: self.created_by,
            created_on: self.created_on,
            global_id: self.global_id,
            content_id: self.content_id,
            labels: self.labels,
            properties: self.properties,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateArtifactMetadataBody {
    name: Option<String>,
    description: Option<String>,
    labels: Option<Vec<String>>,
    properties: Option<HashMap<String, String>>,
}
