use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Api, Coin, Order, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

pub static NAMESPACE_ORDER_BID: &[u8] = b"bid";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[deprecated(since = "0.15.0")]
pub struct BidOrder {
    pub base: String,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: Coin,
    pub size: Uint128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidOrderV1 {
    pub base: String,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: String,
    pub quote_size: Uint128,
    pub size: Uint128,
}

#[allow(deprecated)]
impl From<BidOrder> for BidOrderV1 {
    fn from(bid_order: BidOrder) -> Self {
        BidOrderV1 {
            id: bid_order.id,
            owner: bid_order.owner,
            base: bid_order.base,
            quote: bid_order.quote.denom,
            quote_size: bid_order.quote.amount,
            price: bid_order.price,
            size: bid_order.size,
        }
    }
}

#[allow(deprecated)]
pub fn migrate_bid_orders(
    store: &mut dyn Storage,
    _api: &dyn Api,
    _msg: &MigrateMsg,
) -> Result<(), ContractError> {
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // version support added in 0.15.0, all previous versions migrate to v1 of state data
    let upgrade_req = VersionReq::parse("<0.15.0")?;

    if upgrade_req.matches(&current_version) {
        let legacy_bid_storage: Bucket<BidOrder> = bucket(store, NAMESPACE_ORDER_BID);

        let migrated_bid_orders: Vec<Result<(Vec<u8>, BidOrderV1), ContractError>> =
            legacy_bid_storage
                .range(None, None, Order::Ascending)
                .map(|kv_bid| -> Result<(Vec<u8>, BidOrderV1), ContractError> {
                    let (bid_key, bid) = kv_bid?;
                    Ok((bid_key, bid.into()))
                })
                .collect();

        let mut bid_storage = get_bid_storage(store);
        for migrated_bid_order in migrated_bid_orders {
            let (bid_key, bid) = migrated_bid_order?;
            bid_storage.save(&bid_key, &bid)?
        }
    }

    Ok(())
}

#[allow(deprecated)]
pub fn get_legacy_bid_storage(storage: &mut dyn Storage) -> Bucket<BidOrder> {
    bucket(storage, NAMESPACE_ORDER_BID)
}

pub fn get_bid_storage(storage: &mut dyn Storage) -> Bucket<BidOrderV1> {
    bucket(storage, NAMESPACE_ORDER_BID)
}

pub fn get_bid_storage_read(storage: &dyn Storage) -> ReadonlyBucket<BidOrderV1> {
    bucket_read(storage, NAMESPACE_ORDER_BID)
}

#[cfg(test)]
mod tests {
    use crate::bid_order::{
        get_bid_storage_read, migrate_bid_orders, BidOrderV1, NAMESPACE_ORDER_BID,
    };
    use crate::contract_info::set_legacy_contract_info;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::{bid_order, contract_info};
    use cosmwasm_std::{coin, Addr, Uint128};
    use cosmwasm_storage::{bucket, Bucket};
    use provwasm_mocks::mock_dependencies;

    #[test]
    #[allow(deprecated)]
    pub fn migrate_legacy_bid_to_v1() -> Result<(), ContractError> {
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

        let mut legacy_bid_storage: Bucket<bid_order::BidOrder> =
            bucket(&mut deps.storage, NAMESPACE_ORDER_BID);

        legacy_bid_storage.save(
            b"id",
            &bid_order::BidOrder {
                base: "base_1".to_string(),
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: coin(1000, "quote_1"),
                size: Uint128(100),
            },
        )?;

        migrate_bid_orders(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                fee_rate: None,
                fee_account: None,
            },
        )?;

        let bid_storage = get_bid_storage_read(&deps.storage);
        let migrated_bid = bid_storage.load(b"id")?;

        assert_eq!(
            migrated_bid,
            BidOrderV1 {
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".to_string(),
                quote: "quote_1".to_string(),
                quote_size: Uint128(1000),
                price: "10".to_string(),
                size: Uint128(100)
            }
        );

        Ok(())
    }
}
