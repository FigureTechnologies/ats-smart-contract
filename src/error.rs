use cosmwasm_std::StdError;
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

    #[error("Cannot send funds when canceling order")]
    CancelWithFunds,

    #[error("Cannot send funds when executing match")]
    ExecuteWithFunds,

    #[error("Execute price must be either the ask or bid price")]
    ExecutePriceInvalid,

    #[error("Inconvertible base denomination")]
    InconvertibleBaseDenom,

    #[error("Invalid fields: {fields:?}")]
    InvalidFields { fields: Vec<String> },

    #[error("Failed to load order: {error:?}")]
    OrderLoad { error: StdError },

    #[error("Total (price * size) exceeds max allowed")]
    TotalOverflow,

    #[error("Total (price * size) must be an integer")]
    NonIntegerTotal,

    #[error("One quote required in order")]
    QuoteQuantity,

    #[error("Sent funds does not match order")]
    SentFundsOrderMismatch,

    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Unsupported quote denomination")]
    UnsupportedQuoteDenom,
}

impl From<ContractError> for StdError {
    fn from(_: ContractError) -> Self {
        StdError::ParseErr {
            target_type: "".to_string(),
            msg: "".to_string(),
        }
    }
}
