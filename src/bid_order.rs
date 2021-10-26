use crate::common::{Base, Quote};
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

#[deprecated(since = "0.16.1")]
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidOrderV2 {
    pub base: Base,
    pub fee_size: Option<Uint128>,
    pub fee_filled: Option<Uint128>,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: Quote,
}

#[allow(deprecated)]
impl From<BidOrder> for BidOrderV2 {
    fn from(bid_order: BidOrder) -> Self {
        BidOrderV2 {
            base: Base {
                denom: bid_order.base,
                filled: Uint128::zero(),
                size: bid_order.size,
            },
            fee_filled: None,
            fee_size: None,
            id: bid_order.id,
            owner: bid_order.owner,
            price: bid_order.price,
            quote: Quote {
                denom: bid_order.quote.denom,
                filled: Uint128::zero(),
                size: bid_order.quote.amount,
            },
        }
    }
}

#[allow(deprecated)]
impl From<BidOrderV1> for BidOrderV2 {
    fn from(bid_order: BidOrderV1) -> Self {
        BidOrderV2 {
            base: Base {
                denom: bid_order.base,
                filled: Uint128::zero(),
                size: bid_order.size,
            },
            id: bid_order.id,
            fee_size: None,
            fee_filled: None,
            owner: bid_order.owner,
            price: bid_order.price,
            quote: Quote {
                denom: bid_order.quote,
                filled: Uint128::zero(),
                size: bid_order.quote_size,
            },
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

        let migrated_bid_orders: Vec<Result<(Vec<u8>, BidOrderV2), ContractError>> =
            legacy_bid_storage
                .range(None, None, Order::Ascending)
                .map(|kv_bid| -> Result<(Vec<u8>, BidOrderV2), ContractError> {
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

    // migration from 0.15.2 - 0.16.0 => 0.16.1
    if VersionReq::parse(">=0.15.1, <0.16.1")?.matches(&current_version) {
        let bid_order_v1_storage: Bucket<BidOrderV1> = bucket(store, NAMESPACE_ORDER_BID);

        let migrated_bid_orders: Vec<Result<(Vec<u8>, BidOrderV2), ContractError>> =
            bid_order_v1_storage
                .range(None, None, Order::Ascending)
                .map(|kv_bid| -> Result<(Vec<u8>, BidOrderV2), ContractError> {
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

pub fn get_bid_storage(storage: &mut dyn Storage) -> Bucket<BidOrderV2> {
    bucket(storage, NAMESPACE_ORDER_BID)
}

pub fn get_bid_storage_read(storage: &dyn Storage) -> ReadonlyBucket<BidOrderV2> {
    bucket_read(storage, NAMESPACE_ORDER_BID)
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use crate::bid_order::{
        get_bid_storage_read, migrate_bid_orders, BidOrderV1, BidOrderV2, NAMESPACE_ORDER_BID,
    };
    use crate::common::{Base, Quote};
    use crate::contract_info::set_legacy_contract_info;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1};
    use crate::{bid_order, contract_info};
    use cosmwasm_std::{coin, Addr, Uint128};
    use cosmwasm_storage::{bucket, Bucket};
    use provwasm_mocks::mock_dependencies;

    #[test]
    #[allow(deprecated)]
    pub fn migrate_legacy_bid_to_v2() -> Result<(), ContractError> {
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
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        )?;

        migrate_bid_orders(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
        )?;

        let bid_storage = get_bid_storage_read(&deps.storage);
        let migrated_bid = bid_storage.load(b"id")?;

        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Base {
                    denom: "base_1".to_string(),
                    filled: Uint128::zero(),
                    size: Uint128::new(100)
                },
                fee_filled: None,
                fee_size: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Quote {
                    denom: "quote_1".to_string(),
                    filled: Uint128::zero(),
                    size: Uint128::new(1000),
                },
            }
        );

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    pub fn migrate_bid_v1_to_v2() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.0".to_string(),
            },
        )?;

        let mut bid_v1_storage: Bucket<bid_order::BidOrderV1> =
            bucket(&mut deps.storage, NAMESPACE_ORDER_BID);

        bid_v1_storage.save(
            b"id",
            &BidOrderV1 {
                base: "base_1".to_string(),
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: "quote_1".to_string(),
                quote_size: Uint128::new(1000),
                size: Uint128::new(100),
            },
        )?;

        migrate_bid_orders(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
        )?;

        let bid_storage = get_bid_storage_read(&deps.storage);
        let migrated_bid = bid_storage.load(b"id")?;

        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Base {
                    denom: "base_1".to_string(),
                    filled: Uint128::zero(),
                    size: Uint128::new(100),
                },
                fee_filled: None,
                fee_size: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Quote {
                    denom: "quote_1".to_string(),
                    filled: Uint128::zero(),
                    size: Uint128::new(1000),
                },
            }
        );

        Ok(())
    }
}
