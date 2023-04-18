use crate::constants::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct InstantiateMsg {
    name: String,
    bind_name: String,
    base_denom: String,
    convertible_base_denoms: Vec<String>,
    supported_quote_denoms: Vec<String>,
    approvers: Vec<String>,
    executors: Vec<String>,
    ask_required_attributes: Vec<String>,
    bid_required_attributes: Vec<String>,
    price_precision: String,
    size_increment: String,
}

impl InstantiateMsg {
    /// Instantiate the start contract with the following parameters:
    ///
    ///   {
    ///       "name":"'$SERVICE_NAME'",
    ///       "bind_name":"'$SERVICE_NAME'",
    ///       "base_denom":"gme.local",
    ///       "convertible_base_denoms":[],
    ///       "supported_quote_denoms":["usd.local"],
    ///       "approvers":[],
    ///       "executors":["'$node0'"],
    ///       "ask_required_attributes":[],
    ///       "bid_required_attributes":[],
    ///       "price_precision": "0",
    ///       "size_increment": "1"
    ///   }'
    pub(crate) fn build<S: Into<String>>(node0_address: &str, service_name: S) -> InstantiateMsg {
        let service_name = service_name.into();
        InstantiateMsg {
            name: service_name.clone(),
            bind_name: service_name,
            base_denom: BASE_DENOM.to_string(),
            convertible_base_denoms: vec![],
            supported_quote_denoms: vec![QUOTE_DENOM.to_string()],
            approvers: vec![],
            executors: vec![node0_address.to_owned()],
            ask_required_attributes: vec![],
            bid_required_attributes: vec![],
            price_precision: "0".to_string(),
            size_increment: "1".to_string(),
        }
    }
}
