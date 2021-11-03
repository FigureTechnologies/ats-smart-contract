use crate::common::{Action, Event};
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Api, Coin, Order, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BidOrderV2 {
    pub base: Coin,
    pub events: Vec<Event>,
    pub fee: Option<Coin>,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: Coin,
}

impl BidOrderV2 {
    pub fn calculate_fee(
        &self,
        gross_quote_proceeds: Uint128,
    ) -> Result<Option<Coin>, ContractError> {
        match &self.fee {
            Some(bid_order_fee) => {
                // calculate expected ratio of quote remaining after this transaction
                let expected_quote_ratio =
                    self.get_quote_ratio(self.get_remaining_quote() - gross_quote_proceeds);

                // calculate expected remaining fee
                let expected_remaining_fee = expected_quote_ratio
                    .checked_mul(Decimal::from(bid_order_fee.amount.u128()))
                    .ok_or(ContractError::TotalOverflow)?
                    .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                    .to_u128()
                    .ok_or(ContractError::TotalOverflow)?;

                // the bid fee due is the difference between the expected remaining fee and the current remaining fee
                let bid_fee = self
                    .get_remaining_fee()
                    .checked_sub(Uint128::new(expected_remaining_fee))
                    .map_err(|_| ContractError::BidOrderFeeInsufficientFunds)?;

                let bid_fee = Coin {
                    denom: bid_order_fee.denom.to_owned(),
                    amount: bid_fee,
                };

                if bid_fee.amount.gt(&Uint128::zero()) {
                    Ok(Some(bid_fee))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Returns the remaining amount of base in the order
    pub fn get_remaining_base(&self) -> Uint128 {
        self.base.amount
            - self
                .events
                .iter()
                .map(|event| match &event.action {
                    Action::Fill { base, .. } => base.amount,
                    Action::Reject { base, .. } => base.amount,
                    _ => Uint128::zero(),
                })
                .sum::<Uint128>()
    }

    /// Calculates the ratio of an amount to the bid order base amount
    pub fn get_base_ratio(&self, amount: Uint128) -> Decimal {
        Decimal::from_u128(amount.u128())
            .unwrap()
            .checked_div(Decimal::from_u128(self.base.amount.u128()).unwrap())
            .unwrap()
    }

    /// Returns the remaining amount of fee in the order
    pub fn get_remaining_fee(&self) -> Uint128 {
        match &self.fee {
            None => Uint128::zero(),
            Some(fee) => {
                fee.amount
                    - self
                        .events
                        .iter()
                        .map(|event| match &event.action {
                            Action::Fill { fee, .. } => match fee {
                                None => Uint128::zero(),
                                Some(fee) => fee.amount,
                            },
                            Action::Refund { fee, .. } => match fee {
                                None => Uint128::zero(),
                                Some(fee) => fee.amount,
                            },
                            Action::Reject { fee, .. } => match fee {
                                None => Uint128::zero(),
                                Some(fee) => fee.amount,
                            },
                        })
                        .sum::<Uint128>()
            }
        }
    }

    /// Calculates the ratio of an amount to the bid order quote amount
    pub fn get_quote_ratio(&self, amount: Uint128) -> Decimal {
        Decimal::from_u128(amount.u128())
            .unwrap()
            .checked_div(Decimal::from_u128(self.quote.amount.u128()).unwrap())
            .unwrap()
    }

    /// Returns the remaining amount of quote in the order
    pub fn get_remaining_quote(&self) -> Uint128 {
        self.quote.amount
            - self
                .events
                .iter()
                .map(|event| match &event.action {
                    Action::Fill { quote, .. } => quote.amount,
                    Action::Refund { quote, .. } => quote.amount,
                    Action::Reject { quote, .. } => quote.amount,
                })
                .sum::<Uint128>()
    }
}

#[allow(deprecated)]
impl From<BidOrder> for BidOrderV2 {
    fn from(bid_order: BidOrder) -> Self {
        BidOrderV2 {
            base: Coin {
                amount: bid_order.size,
                denom: bid_order.base,
            },
            events: vec![],
            fee: None,
            id: bid_order.id,
            owner: bid_order.owner,
            price: bid_order.price,
            quote: Coin {
                amount: bid_order.quote.amount,
                denom: bid_order.quote.denom,
            },
        }
    }
}

#[allow(deprecated)]
impl From<BidOrderV1> for BidOrderV2 {
    fn from(bid_order: BidOrderV1) -> Self {
        BidOrderV2 {
            base: Coin {
                amount: bid_order.size,
                denom: bid_order.base,
            },
            id: bid_order.id,
            fee: None,
            owner: bid_order.owner,
            price: bid_order.price,
            quote: Coin {
                amount: bid_order.quote_size,
                denom: bid_order.quote,
            },
            events: vec![],
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

    // migration from 0.15.2 - 0.16.1 => 0.16.2
    if VersionReq::parse(">=0.15.1, <0.16.2")?.matches(&current_version) {
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
    use crate::common::{Action, Event};
    use crate::contract_info::set_legacy_contract_info;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1};
    use crate::{bid_order, contract_info};
    use cosmwasm_std::{coin, Addr, Coin, Uint128};
    use cosmwasm_storage::{bucket, Bucket};
    use provwasm_mocks::mock_dependencies;
    use rust_decimal::prelude::FromStr;
    use rust_decimal::Decimal;

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
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".to_string(),
                },
                events: vec![],
                fee: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Coin {
                    amount: Uint128::new(1000),
                    denom: "quote_1".to_string(),
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
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".to_string(),
                },
                events: vec![],
                fee: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Coin {
                    amount: Uint128::new(1000),
                    denom: "quote_1".to_string(),
                },
            }
        );

        Ok(())
    }

    #[test]
    pub fn get_functions() -> Result<(), ContractError> {
        let bid_order = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(100),
                denom: "base_1".to_string(),
            },
            events: vec![
                Event {
                    action: Action::Fill {
                        base: Coin {
                            denom: "base_1".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Some(Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(2),
                        }),
                        price: "2".to_string(),
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(20),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Refund {
                        fee: Some(Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(8),
                        }),
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(80),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Reject {
                        base: Coin {
                            denom: "base_1".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Default::default(),
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(100),
                        },
                    },
                    block_info: Default::default(),
                },
            ],
            fee: None,
            id: "id".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "10".to_string(),
            quote: Coin {
                amount: Uint128::new(1000),
                denom: "quote_1".to_string(),
            },
        };

        assert_eq!(bid_order.get_remaining_base(), Uint128::new(80));
        assert_eq!(
            bid_order.get_base_ratio(bid_order.get_remaining_base()),
            Decimal::from_str("0.8").unwrap()
        );
        assert_eq!(bid_order.get_remaining_quote(), Uint128::new(800));
        assert_eq!(
            bid_order.get_quote_ratio(bid_order.get_remaining_quote()),
            Decimal::from_str("0.8").unwrap()
        );

        Ok(())
    }
}
