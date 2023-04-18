use thiserror::Error as ThisError;

pub(crate) type Result<T> = anyhow::Result<T>;

#[derive(ThisError, Debug)]
pub(crate) enum ProfilerError {
    #[error("provenance error: {0}")]
    ProvenanceError(String),
    #[error("cosm-orc chain error")]
    CosmOrcChainError(#[from] cosm_orc::orchestrator::error::ChainError),
    #[error("cosm-orc process error")]
    CosmOrcProcessError(#[from] cosm_orc::orchestrator::error::ProcessError),
    #[error("cosm-orc store error")]
    CosmOrcStoreError(#[from] cosm_orc::orchestrator::error::StoreError),
    #[error("missing mnemonic: {0}")]
    MissingMnemonic(String),
    #[error("missing store code response")]
    MissingStoreCodeResponse,
    #[error("missing wasm directory")]
    MissingWasmDirectory,
    #[error("serde json error")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("utf8 error")]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub(crate) fn provenance_error<S: Into<String>, T>(msg: S) -> Result<T> {
    Err(ProfilerError::ProvenanceError(msg.into()).into())
}
