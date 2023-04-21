use crate::contract_info::require_version;
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Coin, DepsMut, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use provwasm_std::ProvenanceQuery;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub static NAMESPACE_ORDER_ASK: &[u8] = b"ask";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderStatus {
    PendingIssuerApproval,
    Ready {
        approver: Addr,
        converted_base: Coin,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderClass {
    Basic,
    Convertible { status: AskOrderStatus },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AskOrderV1 {
    pub id: String,
    pub owner: Addr,
    pub class: AskOrderClass,
    pub base: String,
    pub quote: String,
    pub price: String,
    pub size: Uint128,
}

pub fn get_ask_storage(storage: &mut dyn Storage) -> Bucket<AskOrderV1> {
    bucket(storage, NAMESPACE_ORDER_ASK)
}

pub fn get_ask_storage_read(storage: &dyn Storage) -> ReadonlyBucket<AskOrderV1> {
    bucket_read(storage, NAMESPACE_ORDER_ASK)
}

pub fn migrate_ask_orders(
    deps: DepsMut<ProvenanceQuery>,
    _msg: &MigrateMsg,
) -> Result<(), ContractError> {
    let store = deps.storage;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    require_version(">=0.15.0", &current_version)?;

    Ok(())
}

#[cfg(test)]
mod tests {}
