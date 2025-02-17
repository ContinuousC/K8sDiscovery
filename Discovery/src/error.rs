/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("standard error: {0}")]
    Std(#[from] std::io::Error),
    #[error("arg parser error: {0}")]
    Parsing(#[from] clap::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("failed to parse url: {0}")]
    Url(#[from] url::ParseError),
    #[error("request error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("kube error: {0}")]
    Kube(#[from] kube::Error),
    #[error("kube runtime error: {0}")]
    KubeRuntime(#[from] kube::runtime::watcher::Error),
    #[error("opensearch error: {0}")]
    Opensearch(#[from] opensearch::Error),
    #[error("builder error: {0}")]
    Builder(#[from] opensearch::http::transport::BuildError),
    #[error("relation graph error: {0}: {1}")]
    RelationGraph(reqwest::Error, String),
    #[error("failed to read token secret: {0}")]
    ReadTokenSecret(std::io::Error),
    #[error("token request failed: {0}")]
    TokenRequest(reqwest::Error),
    #[error("failed to read ca certificate: {0}")]
    ReadCaCert(std::io::Error),
    #[error("failed to load ca cert: {0}")]
    LoadCaCert(reqwest::Error),
    #[error("failed to build relation graph engine http client: {0}")]
    BuildEngineClient(reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
