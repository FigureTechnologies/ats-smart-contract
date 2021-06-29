use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Api, Coin, Order, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use schemars::JsonSchema;
use semver::{Version, VersionReq};
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
#[deprecated(since = "0.15.0")]
pub struct AskOrder {
    pub base: Coin,
    pub class: AskOrderClass,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: String,
    pub size: Uint128,
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

#[allow(deprecated)]
impl From<AskOrder> for AskOrderV1 {
    fn from(ask_order: AskOrder) -> Self {
        AskOrderV1 {
            id: ask_order.id,
            owner: ask_order.owner,
            class: ask_order.class,
            base: ask_order.base.denom,
            quote: ask_order.quote,
            price: ask_order.price,
            size: ask_order.base.amount,
        }
    }
}

#[allow(deprecated)]
pub fn migrate_ask_orders(
    store: &mut dyn Storage,
    _api: &dyn Api,
    _msg: &MigrateMsg,
) -> Result<(), ContractError> {
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // version support added in 0.15.0, all previous versions migrate to v1 of state data
    let upgrade_req = VersionReq::parse("<0.15.0")?;

    if upgrade_req.matches(&current_version) {
        let legacy_ask_storage: Bucket<AskOrder> = bucket(store, NAMESPACE_ORDER_ASK);

        let migrated_ask_orders: Vec<Result<(Vec<u8>, AskOrderV1), ContractError>> =
            legacy_ask_storage
                .range(None, None, Order::Ascending)
                .map(|kv_ask| -> Result<(Vec<u8>, AskOrderV1), ContractError> {
                    let (ask_key, ask) = kv_ask?;
                    Ok((ask_key, ask.into()))
                })
                .collect();

        let mut ask_storage = get_ask_storage(store);
        for migrated_ask_order in migrated_ask_orders {
            let (ask_key, ask) = migrated_ask_order?;
            ask_storage.save(&ask_key, &ask)?
        }
    }

    Ok(())
}

#[allow(deprecated)]
pub fn get_legacy_ask_storage(storage: &mut dyn Storage) -> Bucket<AskOrder> {
    bucket(storage, NAMESPACE_ORDER_ASK)
}

pub fn get_ask_storage(storage: &mut dyn Storage) -> Bucket<AskOrderV1> {
    bucket(storage, NAMESPACE_ORDER_ASK)
}

pub fn get_ask_storage_read(storage: &dyn Storage) -> ReadonlyBucket<AskOrderV1> {
    bucket_read(storage, NAMESPACE_ORDER_ASK)
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use crate::ask_order::{
        get_ask_storage_read, migrate_ask_orders, AskOrder, AskOrderClass, AskOrderV1,
        NAMESPACE_ORDER_ASK,
    };
    use crate::contract_info;
    use crate::contract_info::set_legacy_contract_info;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use cosmwasm_std::{coin, Addr, Uint128};
    use cosmwasm_storage::{bucket, Bucket};
    use provwasm_mocks::mock_dependencies;

    #[test]
    #[allow(deprecated)]
    pub fn migrate_legacy_ask_to_v1() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        set_legacy_contract_info(
            &mut deps.storage,
            &contract_info::ContractInfo {
                name: "contract_name".to_string(),
                definition: "contract_def".to_string(),
                version: "0.14.99".to_string(),
                bind_name: "bind_name".to_string(),
                base_denom: "base_1".to_string(),
                convertible_base_denoms: vec![],
                supported_quote_denoms: vec![],
                executors: vec![],
                issuers: vec![],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        )?;

        let mut legacy_ask_storage: Bucket<AskOrder> =
            bucket(&mut deps.storage, NAMESPACE_ORDER_ASK);

        legacy_ask_storage.save(
            b"id",
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "id".to_string(),
                owner: Addr::unchecked("asker"),
                price: "10".to_string(),
                quote: "quote_1".to_string(),
                size: Uint128(100),
            },
        )?;

        migrate_ask_orders(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg { approvers: vec![] },
        )?;

        let ask_storage = get_ask_storage_read(&deps.storage);
        let migrated_ask = ask_storage.load(b"id")?;

        assert_eq!(
            migrated_ask,
            AskOrderV1 {
                id: "id".to_string(),
                owner: Addr::unchecked("asker"),
                class: AskOrderClass::Basic,
                base: "base_1".to_string(),
                quote: "quote_1".to_string(),
                price: "10".to_string(),
                size: Uint128(100)
            }
        );

        Ok(())
    }
}
