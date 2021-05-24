use crate::error::ContractError;
use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub name: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub executors: Vec<String>,
    pub issuer: String,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
    pub price_precision: Uint128,
    pub size_increment: Uint128,
}

/// Simple validation of InstantiateMsg data
///
/// ### Example
///
/// ```rust
/// use ats_smart_contract::msg::{InstantiateMsg, Validate};
/// pub fn instantiate(msg: InstantiateMsg){
///
///     let result = msg.validate();
/// }
/// ```
impl Validate for InstantiateMsg {
    fn validate(&self) -> Result<(), ContractError> {
        let mut invalid_fields: Vec<&str> = vec![];

        if self.name.is_empty() {
            invalid_fields.push("name");
        }
        if self.bind_name.is_empty() {
            invalid_fields.push("bind_name");
        }
        if self.base_denom.is_empty() {
            invalid_fields.push("base_denom");
        }
        if self.supported_quote_denoms.is_empty() {
            invalid_fields.push("supported_quote_denoms");
        }
        if self.executors.is_empty() {
            invalid_fields.push("executors");
        }
        if self.price_precision.lt(&Uint128(0)) || self.price_precision.gt(&Uint128(18)) {
            invalid_fields.push("price_precision");
        }
        if self.size_increment.lt(&Uint128(1)) {
            invalid_fields.push("size_increment");
        }

        match invalid_fields.len() {
            0 => Ok(()),
            _ => Err(ContractError::InvalidFields {
                fields: invalid_fields.into_iter().map(|item| item.into()).collect(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ApproveAsk {
        id: String,
    },
    CancelAsk {
        id: String,
    },
    CancelBid {
        id: String,
    },
    CreateAsk {
        id: String,
        quote: String,
        price: String,
    },
    CreateBid {
        id: String,
        base: String,
        price: String,
        size: Uint128,
    },
    ExecuteMatch {
        ask_id: String,
        bid_id: String,
        price: String,
        size: Uint128,
    },
}

impl Validate for ExecuteMsg {
    /// Simple validation of ExecuteMsg data
    ///
    /// ### Example
    ///
    /// ```rust
    /// use ats_smart_contract::msg::{ExecuteMsg, Validate};
    ///
    /// pub fn execute(msg: ExecuteMsg){
    ///     let result = msg.validate();
    ///     todo!()
    /// }
    /// ```
    fn validate(&self) -> Result<(), ContractError> {
        let mut invalid_fields: Vec<&str> = vec![];

        match self {
            ExecuteMsg::ApproveAsk { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::CreateAsk { id, quote, price } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if price.is_empty() {
                    invalid_fields.push("price");
                }
                if quote.is_empty() {
                    invalid_fields.push("quote");
                }
            }
            ExecuteMsg::CreateBid {
                id,
                base,
                price,
                size,
            } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if base.is_empty() {
                    invalid_fields.push("base");
                }
                if price.is_empty() {
                    invalid_fields.push("price");
                }
                if size.lt(&Uint128(1)) {
                    invalid_fields.push("size");
                }
            }
            ExecuteMsg::CancelAsk { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::CancelBid { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::ExecuteMatch {
                ask_id,
                bid_id,
                price,
                size,
            } => {
                if Uuid::parse_str(ask_id).is_err() {
                    invalid_fields.push("ask_id");
                }
                if Uuid::parse_str(bid_id).is_err() {
                    invalid_fields.push("bid_id");
                }
                if price.is_empty() {
                    invalid_fields.push("price");
                }
                if size.lt(&Uint128(1)) {
                    invalid_fields.push("price");
                }
            }
        }

        match invalid_fields.len() {
            0 => Ok(()),
            _ => Err(ContractError::InvalidFields {
                fields: invalid_fields.into_iter().map(|item| item.into()).collect(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetAsk { id: String },
    GetBid { id: String },
    GetContractInfo {},
}

impl Validate for QueryMsg {
    /// Simple validation of QueryMsg data
    ///
    /// ### Example
    ///
    /// ```rust
    /// use ats_smart_contract::msg::{QueryMsg, Validate};
    /// pub fn query(msg: QueryMsg){
    ///
    ///     let result = msg.validate();
    /// }
    /// ```
    fn validate(&self) -> Result<(), ContractError> {
        let mut invalid_fields: Vec<&str> = vec![];

        match self {
            QueryMsg::GetAsk { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            QueryMsg::GetBid { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            QueryMsg::GetContractInfo {} => {}
        }

        match invalid_fields.len() {
            0 => Ok(()),
            _ => Err(ContractError::InvalidFields {
                fields: invalid_fields.into_iter().map(|item| item.into()).collect(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {
    Migrate {},
}

impl Validate for MigrateMsg {
    /// Simple validation of MigrateMsg data
    ///
    /// ### Example
    ///
    /// ```rust
    /// use ats_smart_contract::msg::{MigrateMsg, Validate};
    /// pub fn query(msg: MigrateMsg){
    ///
    ///     let result = msg.validate();
    /// }
    /// ```
    fn validate(&self) -> Result<(), ContractError> {
        Ok(())
    }
}

pub trait Validate {
    fn validate(&self) -> Result<(), ContractError>;
}
