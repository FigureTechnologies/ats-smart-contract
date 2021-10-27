use cosmwasm_std::StdError;
use semver::Error as SemverError;
use serde_json::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Ask order price does not match Bid order price")]
    AskBidPriceMismatch,

    #[error("Ask order not ready: {current_status:?}")]
    AskOrderNotReady { current_status: String },

    #[error("One base required in order")]
    BaseQuantity,

    #[error("Bid order not found")]
    BidOrderNotFound,

    #[error("Bid order does not have sufficient fee funds")]
    BidOrderFeeInsufficientFunds,

    #[error("Cannot send funds when canceling order")]
    CancelWithFunds,

    #[error("Cannot send funds when executing match")]
    ExecuteWithFunds,

    #[error("Cannot send funds when expiring order")]
    ExpireWithFunds,

    #[error("Fee size is not: {fee_rate:?}% of total")]
    InvalidFeeSize { fee_rate: String },

    #[error("Inconvertible base denomination")]
    InconvertibleBaseDenom,

    #[error("Execute price must be either the ask or bid price")]
    InvalidExecutePrice,

    #[error("Execute size must be either the ask or bid price")]
    InvalidExecuteSize,

    #[error("Invalid fields: {fields:?}")]
    InvalidFields { fields: Vec<String> },

    #[error("Size increment must be a multiple of (10 ^ price precision)")]
    InvalidPricePrecisionSizePair,

    #[error("Failed to load order: {error:?}")]
    LoadOrderFailed { error: StdError },

    #[error("Total (price * size) must be an integer")]
    NonIntegerTotal,

    #[error("One quote required in order")]
    QuoteQuantity,

    #[error("Sent funds does not match order")]
    SentFundsOrderMismatch,

    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    JsonSerde(#[from] Error),

    #[error("{0}")]
    SemverError(#[from] SemverError),

    #[error("Total (price * size) exceeds max allowed")]
    TotalOverflow,

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Unsupported quote denomination")]
    UnsupportedQuoteDenom,

    #[error("Unsupported upgrade: {source_version:?} => {target_version:?}")]
    UnsupportedUpgrade {
        source_version: String,
        target_version: String,
    },
}

impl From<ContractError> for StdError {
    fn from(error: ContractError) -> Self {
        StdError::GenericErr {
            msg: error.to_string(),
        }
    }
}
