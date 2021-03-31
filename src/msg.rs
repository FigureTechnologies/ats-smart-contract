use crate::error::ContractError;
use cosmwasm_std::{Coin, HumanAddr, Uint128};
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

impl Validate for InstantiateMsg {
    /// Simple validation of InstantiateMsg data
    ///
    /// ### Example
    /// ```
    /// let result = execute_msg.validate();
    /// ```
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
    CancelAsk { id: String },
    CancelBid { id: String },
    CreateAsk { id: String, quote: Coin },
    CreateBid { id: String, base: Coin },
    ApproveAsk { id: String },
    RejectAsk { id: String },
    ExpireAsk { id: String },
    ExpireBid { id: String },
    ExecuteMatch { ask_id: String, bid_id: String },
}

impl Validate for ExecuteMsg {
    /// Simple validation of ExecuteMsg data
    ///
    /// ### Example
    /// ```
    /// let result = execute_msg.validate();
    /// ```
    fn validate(&self) -> Result<(), ContractError> {
        let mut invalid_fields: Vec<&str> = vec![];

        match self {
            ExecuteMsg::CreateAsk { id, quote } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
                if quote.amount.eq(&Uint128::zero()) {
                    invalid_fields.push("quote.amount");
                }
                if quote.denom.is_empty() {
                    invalid_fields.push("quote.denom");
                }
            }
            ExecuteMsg::CreateBid { id, base } => {
                if id.is_empty() {
                    invalid_fields.push("id");
                }
                if base.amount.eq(&Uint128::zero()) {
                    invalid_fields.push("base.amount");
                }
                if base.denom.is_empty() {
                    invalid_fields.push("base.denom");
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
    /// ```
    /// let result = execute_msg.validate();
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
