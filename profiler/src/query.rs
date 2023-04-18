use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum QueryMsg {
    #[allow(dead_code)]
    GetAsk {
        id: String,
    },
    #[allow(dead_code)]
    GetBid {
        id: String,
    },
    GetContractInfo {},
    GetVersionInfo {},
}

#[allow(dead_code)]
pub(crate) fn get_ask<S: Into<String>>(id: S) -> QueryMsg {
    QueryMsg::GetAsk { id: id.into() }
}

#[allow(dead_code)]
pub(crate) fn get_bid<S: Into<String>>(id: S) -> QueryMsg {
    QueryMsg::GetBid { id: id.into() }
}

pub(crate) fn get_contract_info() -> QueryMsg {
    QueryMsg::GetContractInfo {}
}

pub(crate) fn get_contract_version() -> QueryMsg {
    QueryMsg::GetVersionInfo {}
}
