use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::Error;
use std::fmt::{self, Display, Formatter};
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait Provider {
    async fn system_info(&self) -> Result<SystemInfo, Error>;
    async fn fetch_artifact_metadata(&self, group_id: &str, artifact_id: &str) -> Result<ArtifactMetadata, Error>;
    async fn fetch_artifact_version_metadata(&self, group_id: &str, artifact_id: &str, version: &str) -> Result<ArtifactVersionMetadata, Error>;
    async fn fetch_artifact_by_global_id(&self, global_id: u64) -> Result<Vec<u8>, Error>;
    async fn push_artifact(&self, group_id: &str, artifact_id: &str, artifact_type: Option<ArtifactType>, content: Vec<u8>) -> Result<(), Error>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ArtifactType {
    Avro,
    Protobuf,
    Json,
    KConnect,
    OpenAPI,
    AsyncAPI,
    GraphQL,
    Wsdl,
    Xsd,
}

impl Display for ArtifactType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = match self {
            ArtifactType::Avro => "AVRO",
            ArtifactType::Protobuf => "PROTOBUF",
            ArtifactType::Json => "JSON",
            ArtifactType::KConnect => "KCONNECT",
            ArtifactType::OpenAPI => "OPENAPI",
            ArtifactType::AsyncAPI => "ASYNCAPI",
            ArtifactType::GraphQL => "GRAPHQL",
            ArtifactType::Wsdl => "WSDL",
            ArtifactType::Xsd => "XSD",
        };
        s.fmt(f)
    }
}

#[derive(Debug)]
pub struct SystemInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub built_on: String,
}

#[derive(Debug)]
pub struct ArtifactMetadata {
    pub group_id: String,
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub artifact_type: ArtifactType,
    pub version: String,
    pub created_by: String,
    pub created_on: String,
    pub modified_by: String,
    pub modified_on: String,
    pub global_id: u64,
    pub content_id: u64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ArtifactVersionMetadata {
    pub group_id: String,
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub artifact_type: ArtifactType,
    pub version: String,
    pub created_by: String,
    pub created_on: String,
    pub global_id: u64,
    pub content_id: u64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, String>,
}

pub struct NoopProvider;

#[async_trait]
impl Provider for NoopProvider {
    async fn system_info(&self) -> Result<SystemInfo, Error> {
        unimplemented!()
    }

    async fn fetch_artifact_metadata(&self, _group_id: &str, _artifact_id: &str) -> Result<ArtifactMetadata, Error> {
        unimplemented!()
    }

    async fn fetch_artifact_version_metadata(&self, _group_id: &str, _artifact_id: &str, _version: &str) -> Result<ArtifactVersionMetadata, Error> {
        unimplemented!()
    }

    async fn fetch_artifact_by_global_id(&self, _global_id: u64) -> Result<Vec<u8>, Error> {
        unimplemented!()
    }

    async fn push_artifact(&self, _group_id: &str, _artifact_id: &str, _artifact_type: Option<ArtifactType>, _content: Vec<u8>) -> Result<(), Error> {
        unimplemented!()
    }
}
