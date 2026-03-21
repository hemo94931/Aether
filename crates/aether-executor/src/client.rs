use std::path::PathBuf;

use aether_contracts::{ExecutionPlan, ExecutionResult, StreamFrame};

use crate::{ExecutorClientError, TransportMode};

#[derive(Debug, Clone)]
pub struct ExecutorClientConfig {
    pub transport: TransportMode,
    pub endpoint: Option<String>,
    pub socket_path: Option<PathBuf>,
}

impl ExecutorClientConfig {
    pub fn unix_socket(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            transport: TransportMode::UnixSocketHttp,
            endpoint: None,
            socket_path: Some(socket_path.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutorClient {
    config: ExecutorClientConfig,
}

impl ExecutorClient {
    pub fn new(config: ExecutorClientConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &ExecutorClientConfig {
        &self.config
    }

    pub async fn execute(
        &self,
        _plan: &ExecutionPlan,
    ) -> Result<ExecutionResult, ExecutorClientError> {
        if self.config.endpoint.is_none() && self.config.socket_path.is_none() {
            return Err(ExecutorClientError::MissingEndpoint);
        }
        Err(ExecutorClientError::Unimplemented)
    }

    pub async fn execute_stream(
        &self,
        _plan: &ExecutionPlan,
    ) -> Result<Vec<StreamFrame>, ExecutorClientError> {
        if self.config.endpoint.is_none() && self.config.socket_path.is_none() {
            return Err(ExecutorClientError::MissingEndpoint);
        }
        Err(ExecutorClientError::Unimplemented)
    }
}
