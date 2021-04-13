use crate::error::ContractError;
use cosmwasm_std::{HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub name: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub executors: Vec<HumanAddr>,
    pub issuers: Vec<HumanAddr>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
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
    CancelAsk {
        id: String,
    },
    CancelBid {
        id: String,
    },
    CreateAsk {
        id: String,
        quote: String,
        price: Uint128,
    },
    CreateBid {
        id: String,
        base: String,
        price: Uint128,
        size: Uint128,
    },
    ApproveAsk {
        id: String,
    },
    RejectAsk {
        id: String,
    },
    ExpireAsk {
        id: String,
    },
    ExpireBid {
        id: String,
    },
    ExecuteMatch {
        ask_id: String,
        bid_id: String,
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
            ExecuteMsg::CreateAsk { id, quote, price } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
                if price.is_zero() {
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
                if id.is_empty() {
                    invalid_fields.push("id");
                }
                if base.is_empty() {
                    invalid_fields.push("base");
                }
                if price.is_zero() {
                    invalid_fields.push("price");
                }
                if size.is_zero() {
                    invalid_fields.push("size");
                }
            }
            ExecuteMsg::CancelAsk { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::CancelBid { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::ExecuteMatch { ask_id, bid_id } => {
                if ask_id.is_empty() {
                    invalid_fields.push("ask_id");
                }
                if bid_id.is_empty() {
                    invalid_fields.push("bid_id");
                }
            }
            ExecuteMsg::ApproveAsk { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::RejectAsk { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::ExpireAsk { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::ExpireBid { id } => {
                if id.is_empty() {
                    invalid_fields.push("id");
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
                if id.is_empty() {
                    invalid_fields.push("id");
                }
            }
            QueryMsg::GetBid { id } => {
                if id.is_empty() {
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

pub trait Validate {
    fn validate(&self) -> Result<(), ContractError>;
}
