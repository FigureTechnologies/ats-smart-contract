use crate::error::ContractError;
use cosmwasm_std::{Coin, Uint128};
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
    pub approvers: Vec<String>,
    pub executors: Vec<String>,
    pub ask_fee_rate: Option<String>,
    pub ask_fee_account: Option<String>,
    pub bid_fee_rate: Option<String>,
    pub bid_fee_account: Option<String>,
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
        match (&self.ask_fee_rate, &self.ask_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("ask_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("ask_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
        }
        match (&self.bid_fee_rate, &self.bid_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("bid_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("bid_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
        }
        if self.price_precision.lt(&Uint128::new(0)) || self.price_precision.gt(&Uint128::new(18)) {
            invalid_fields.push("price_precision");
        }
        if self.size_increment.lt(&Uint128::new(1)) {
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
        base: String,
        size: Uint128,
    },
    CancelAsk {
        id: String,
    },
    CancelBid {
        id: String,
    },
    CreateAsk {
        id: String,
        base: String,
        quote: String,
        price: String,
        size: Uint128,
    },
    CreateBid {
        id: String,
        base: String,
        fee: Option<Coin>,
        price: String,
        quote: String,
        quote_size: Uint128,
        size: Uint128,
    },
    ExecuteMatch {
        ask_id: String,
        bid_id: String,
        price: String,
        size: Uint128,
    },
    ExpireAsk {
        id: String,
    },
    ExpireBid {
        id: String,
    },
    RejectAsk {
        id: String,
        size: Option<Uint128>,
    },
    RejectBid {
        id: String,
        size: Option<Uint128>,
    },
    ModifyContract {
        approvers: Option<Vec<String>>,
        executors: Option<Vec<String>>,
        ask_fee_rate: Option<String>,
        ask_fee_account: Option<String>,
        bid_fee_rate: Option<String>,
        bid_fee_account: Option<String>,
        ask_required_attributes: Option<Vec<String>>,
        bid_required_attributes: Option<Vec<String>>,
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
            ExecuteMsg::ApproveAsk { id, base, size } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if base.is_empty() {
                    invalid_fields.push("base");
                }
                if size.lt(&Uint128::new(1)) {
                    invalid_fields.push("size");
                }
            }
            ExecuteMsg::CreateAsk {
                id,
                base,
                quote,
                price,
                size,
            } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if base.is_empty() {
                    invalid_fields.push("base");
                }
                if quote.is_empty() {
                    invalid_fields.push("quote");
                }
                if price.is_empty() {
                    invalid_fields.push("price");
                }
                if size.lt(&Uint128::new(1)) {
                    invalid_fields.push("size");
                }
            }
            ExecuteMsg::CreateBid {
                id,
                base,
                fee,
                price,
                quote,
                quote_size,
                size,
            } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if base.is_empty() {
                    invalid_fields.push("base");
                }
                if let Some(fee) = fee {
                    if fee.amount.lt(&Uint128::new(1)) {
                        invalid_fields.push("fee");
                    }
                }
                if price.is_empty() {
                    invalid_fields.push("price");
                }
                if quote.is_empty() {
                    invalid_fields.push("quote");
                }
                if quote_size.lt(&Uint128::new(1)) {
                    invalid_fields.push("quote_size");
                }
                if size.lt(&Uint128::new(1)) {
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
                if size.lt(&Uint128::new(1)) {
                    invalid_fields.push("price");
                }
            }
            ExecuteMsg::ExpireAsk { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::ExpireBid { id } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
            }
            ExecuteMsg::RejectAsk { id, size } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if let Some(size) = size {
                    if size.lt(&Uint128::new(1)) {
                        invalid_fields.push("size");
                    }
                }
            }
            ExecuteMsg::RejectBid { id, size } => {
                if Uuid::parse_str(id).is_err() {
                    invalid_fields.push("id");
                }
                if let Some(size) = size {
                    if size.lt(&Uint128::new(1)) {
                        invalid_fields.push("size");
                    }
                }
            }
            ExecuteMsg::ModifyContract {
                approvers: _,
                executors: _,
                ask_fee_rate,
                ask_fee_account,
                bid_fee_rate,
                bid_fee_account,
                ask_required_attributes: _,
                bid_required_attributes: _,
            } => {
                match (ask_fee_rate, ask_fee_account) {
                    (Some(_), None) => {
                        invalid_fields.push("ask_fee_account");
                    }
                    (None, Some(_)) => {
                        invalid_fields.push("ask_fee_rate");
                    }
                    (Some(_), Some(_)) => (),
                    (None, None) => (),
                }
                match (bid_fee_rate, bid_fee_account) {
                    (Some(_), None) => {
                        invalid_fields.push("bid_fee_account");
                    }
                    (None, Some(_)) => {
                        invalid_fields.push("bid_fee_rate");
                    }
                    (Some(_), Some(_)) => (),
                    (None, None) => (),
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
    GetVersionInfo {},
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
            QueryMsg::GetVersionInfo {} => {}
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
pub struct ModifyContractMsg {
    pub approvers: Option<Vec<String>>,
    pub executors: Option<Vec<String>>,
    pub ask_fee_rate: Option<String>,
    pub ask_fee_account: Option<String>,
    pub bid_fee_rate: Option<String>,
    pub bid_fee_account: Option<String>,
    pub ask_required_attributes: Option<Vec<String>>,
    pub bid_required_attributes: Option<Vec<String>>,
}

impl Validate for ModifyContractMsg {
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
        let mut invalid_fields: Vec<&str> = vec![];

        match &self.approvers {
            Some(vector) => match vector.as_slice().is_empty() {
                true => invalid_fields.push("approvers_empty"),
                _ => (),
            },
            None => (),
        }
        match &self.executors {
            Some(vector) => match vector.as_slice().is_empty() {
                true => invalid_fields.push("executors_empty"),
                _ => (),
            },
            None => (),
        }
        match (&self.ask_fee_rate, &self.ask_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("ask_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("ask_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
        }
        match (&self.bid_fee_rate, &self.bid_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("bid_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("bid_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
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
pub struct MigrateMsg {
    pub approvers: Option<Vec<String>>,
    pub ask_fee_rate: Option<String>,
    pub ask_fee_account: Option<String>,
    pub bid_fee_rate: Option<String>,
    pub bid_fee_account: Option<String>,
    pub ask_required_attributes: Option<Vec<String>>,
    pub bid_required_attributes: Option<Vec<String>>,
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
        let mut invalid_fields: Vec<&str> = vec![];

        match (&self.ask_fee_rate, &self.ask_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("ask_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("ask_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
        }
        match (&self.bid_fee_rate, &self.bid_fee_account) {
            (Some(_), None) => {
                invalid_fields.push("bid_fee_account");
            }
            (None, Some(_)) => {
                invalid_fields.push("bid_fee_rate");
            }
            (Some(_), Some(_)) => (),
            (None, None) => (),
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
