use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Ask Order price does not match Bid Order price")]
    AskBidPriceMismatch,

    #[error("Ask Order not ready: {current_status:?}")]
    AskOrderNotReady { current_status: String },

    #[error("One base required in order")]
    BaseQuantity,

    #[error("Bid order not found")]
    BidOrderNotFound,

    #[error("Cannot send funds when canceling order")]
    CancelWithFunds,

    #[error("Cannot send funds when executing match")]
    ExecuteWithFunds,

    #[error("Inconvertible base denomination")]
    InconvertibleBaseDenom,

    #[error("Invalid fields: {fields:?}")]
    InvalidFields { fields: Vec<String> },

    #[error("Failed to load order: {error:?}")]
    OrderLoad { error: StdError },

    #[error("One quote required in order")]
    QuoteQuantity,

    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Unsupported quote denomination")]
    UnsupportedQuoteDenom,
}
