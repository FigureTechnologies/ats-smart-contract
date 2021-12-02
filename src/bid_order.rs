use crate::common::{Action, Event};
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{
    coin, Addr, BankMsg, Coin, CosmosMsg, DepsMut, Env, Order, Pair, QuerierWrapper, Response,
    StdResult, Storage, Uint128,
};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use provwasm_std::{transfer_marker_coins, Marker, MarkerType, ProvenanceMsg, ProvenanceQuerier};
use rust_decimal::prelude::{FromPrimitive, FromStr, ToPrimitive};
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
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
    pub fn calculate_fee(&self, gross_proceeds: Uint128) -> Result<Option<Coin>, ContractError> {
        match &self.fee {
            Some(bid_order_fee) => {
                // calculate expected ratio of quote remaining after this transaction
                let expected_quote_ratio =
                    self.get_quote_ratio(self.get_remaining_quote() - gross_proceeds);

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
    deps: DepsMut,
    env: Env,
    _msg: &MigrateMsg,
    mut response: Response<ProvenanceMsg>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let store = deps.storage;
    let querier = deps.querier;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // version support added in 0.15.0, all previous versions migrate to v1 of state data
    let upgrade_req = VersionReq::parse("<0.15.0")?;

    if upgrade_req.matches(&current_version) {
        let existing_bid_order_ids: Vec<Vec<u8>> = bucket_read(store, NAMESPACE_ORDER_BID)
            .range(None, None, Order::Ascending)
            .map(|kv_bid: StdResult<Pair<BidOrder>>| {
                let (bid_key, _) = kv_bid.unwrap();
                bid_key
            })
            .collect();

        for existing_bid_order_id in existing_bid_order_ids {
            let existing_bid_order: BidOrder =
                bucket_read(store, NAMESPACE_ORDER_BID).load(&existing_bid_order_id)?;
            get_bid_storage(store).save(&existing_bid_order_id, &existing_bid_order.into())?
        }
    }

    // migration from 0.15.2 - 0.16.1 => latest
    if VersionReq::parse(">=0.15.1, <0.16.2")?.matches(&current_version) {
        let existing_bid_order_ids: Vec<Vec<u8>> = bucket_read(store, NAMESPACE_ORDER_BID)
            .range(None, None, Order::Ascending)
            .map(|kv_bid: StdResult<Pair<BidOrderV1>>| {
                let (bid_key, _) = kv_bid.unwrap();
                bid_key
            })
            .collect();

        for existing_bid_order_id in existing_bid_order_ids {
            let existing_bid_order: BidOrderV1 =
                bucket_read(store, NAMESPACE_ORDER_BID).load(&existing_bid_order_id)?;
            get_bid_storage(store).save(&existing_bid_order_id, &existing_bid_order.into())?
        }
    }

    // get all bid ids
    let existing_bid_order_ids: Vec<Vec<u8>> = get_bid_storage_read(store)
        .range(None, None, Order::Ascending)
        .map(|kv_bid| {
            let (bid_key, _) = kv_bid.unwrap();
            bid_key
        })
        .collect();

    // determine and create a refund if necessary
    for existing_bid_order_id in existing_bid_order_ids {
        let mut existing_bid_order = get_bid_storage_read(store).load(&existing_bid_order_id)?;

        response =
            calculate_migrate_refund(&querier, &env, response, &mut existing_bid_order).unwrap();

        get_bid_storage(store).save(&existing_bid_order_id, &existing_bid_order)?
    }

    Ok(response)
}

fn calculate_migrate_refund(
    querier: &QuerierWrapper,
    env: &Env,
    mut response: Response<ProvenanceMsg>,
    bid_order: &mut BidOrderV2,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let bid_price =
        Decimal::from_str(&bid_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("BidOrder.price")],
        })?;

    // calculate quote total (price * size), error if overflows
    let total = bid_price
        .checked_mul(Decimal::from(bid_order.base.amount.u128()))
        .ok_or(ContractError::TotalOverflow)?;

    let quote =
        Decimal::from_u128(bid_order.quote.amount.u128()).ok_or(ContractError::InvalidFields {
            fields: vec![String::from("BidOrder.quote")],
        })?;

    // if excess funds exist
    if total.lt(&quote) {
        let refund = quote
            .checked_sub(total)
            .ok_or(ContractError::TotalOverflow)?
            .to_u128()
            .ok_or(ContractError::NonIntegerTotal)?;

        // is bid quote a marker
        let is_quote_restricted_marker = matches!(
            ProvenanceQuerier::new(querier).get_marker_by_denom(bid_order.quote.denom.clone()),
            Ok(Marker {
                marker_type: MarkerType::Restricted,
                ..
            })
        );

        match is_quote_restricted_marker {
            true => {
                response = response.add_message(transfer_marker_coins(
                    refund,
                    bid_order.quote.denom.to_string(),
                    bid_order.owner.to_owned(),
                    env.contract.address.to_owned(),
                )?);
            }
            false => {
                response = response.add_message(CosmosMsg::Bank(BankMsg::Send {
                    to_address: bid_order.owner.to_string(),
                    amount: vec![coin(refund, bid_order.quote.denom.to_string())],
                }));
            }
        }

        bid_order.events.push(Event {
            action: Action::Refund {
                fee: None,
                quote: Coin {
                    denom: bid_order.quote.denom.to_string(),
                    amount: Uint128::new(refund),
                },
            },
            block_info: env.block.to_owned().into(),
        })
    }

    Ok(response)
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
    use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{coin, Addr, BankMsg, Coin, CosmosMsg, Response, Uint128};
    use cosmwasm_storage::{bucket, Bucket};
    use provwasm_mocks::mock_dependencies;
    use provwasm_std::{transfer_marker_coins, Marker, MarkerStatus, MarkerType};
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

        let response = Response::new();

        migrate_bid_orders(
            deps.as_mut(),
            mock_env(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
            response,
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
    pub fn migrate_legacy_bid_to_v2_with_refund() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        let test_marker: Marker = Marker {
            address: Addr::unchecked("marker_address"),
            coins: vec![],
            account_number: 0,
            sequence: 0,
            manager: "marker_manager".to_string(),
            permissions: vec![],
            status: MarkerStatus::Active,
            denom: "marker_1".to_string(),
            total_supply: Default::default(),
            marker_type: MarkerType::Restricted,
            supply_fixed: false,
        };
        deps.querier.with_markers(vec![test_marker]);

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
                quote: coin(100, "quote_1"),
                size: Uint128::new(8),
            },
        )?;

        legacy_bid_storage.save(
            b"id2",
            &bid_order::BidOrder {
                base: "base_1".to_string(),
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: coin(120, "marker_1"),
                size: Uint128::new(9),
            },
        )?;

        let mut response = Response::new();

        response = migrate_bid_orders(
            deps.as_mut(),
            mock_env(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
            response,
        )?;

        let bid_storage = get_bid_storage_read(&deps.storage);

        let migrated_bid = bid_storage.load(b"id")?;
        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(8),
                    denom: "base_1".to_string(),
                },
                events: vec![Event {
                    action: Action::Refund {
                        fee: None,
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(20),
                        },
                    },
                    block_info: mock_env().block.into(),
                }],
                fee: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".to_string(),
                },
            }
        );

        let migrated_bid = bid_storage.load(b"id2")?;
        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(9),
                    denom: "base_1".to_string(),
                },
                events: vec![Event {
                    action: Action::Refund {
                        fee: None,
                        quote: Coin {
                            denom: "marker_1".to_string(),
                            amount: Uint128::new(30),
                        },
                    },
                    block_info: mock_env().block.into(),
                }],
                fee: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Coin {
                    amount: Uint128::new(120),
                    denom: "marker_1".to_string(),
                },
            }
        );
        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            response.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "bidder".to_string(),
                amount: vec![coin(20, "quote_1")],
            })
        );
        assert_eq!(
            response.messages[1].msg,
            transfer_marker_coins(
                30,
                "marker_1",
                Addr::unchecked("bidder"),
                Addr::unchecked(MOCK_CONTRACT_ADDR)
            )
            .unwrap(),
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

        let response = Response::new();

        migrate_bid_orders(
            deps.as_mut(),
            mock_env(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
            response,
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
    pub fn migrate_bid_v1_to_v2_with_refund() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        let test_marker: Marker = Marker {
            address: Addr::unchecked("marker_address"),
            coins: vec![],
            account_number: 0,
            sequence: 0,
            manager: "marker_manager".to_string(),
            permissions: vec![],
            status: MarkerStatus::Active,
            denom: "marker_1".to_string(),
            total_supply: Default::default(),
            marker_type: MarkerType::Restricted,
            supply_fixed: false,
        };
        deps.querier.with_markers(vec![test_marker]);

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
                size: Uint128::new(80),
            },
        )?;

        bid_v1_storage.save(
            b"id2",
            &BidOrderV1 {
                base: "base_1".to_string(),
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: "marker_1".to_string(),
                quote_size: Uint128::new(120),
                size: Uint128::new(9),
            },
        )?;

        let mut response = Response::new();

        response = migrate_bid_orders(
            deps.as_mut(),
            mock_env(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
            response,
        )?;

        let bid_storage = get_bid_storage_read(&deps.storage);

        let migrated_bid = bid_storage.load(b"id")?;
        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(80),
                    denom: "base_1".to_string(),
                },
                events: vec![Event {
                    action: Action::Refund {
                        fee: None,
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(200),
                        },
                    },
                    block_info: mock_env().block.into(),
                }],
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

        let migrated_bid = bid_storage.load(b"id2")?;
        assert_eq!(
            migrated_bid,
            BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(9),
                    denom: "base_1".to_string(),
                },
                events: vec![Event {
                    action: Action::Refund {
                        fee: None,
                        quote: Coin {
                            denom: "marker_1".to_string(),
                            amount: Uint128::new(30),
                        },
                    },
                    block_info: mock_env().block.into(),
                }],
                fee: None,
                id: "id".to_string(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_string(),
                quote: Coin {
                    amount: Uint128::new(120),
                    denom: "marker_1".to_string(),
                },
            }
        );
        assert_eq!(response.messages.len(), 2);
        assert_eq!(
            response.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "bidder".to_string(),
                amount: vec![coin(200, "quote_1")],
            })
        );
        assert_eq!(
            response.messages[1].msg,
            transfer_marker_coins(
                30,
                "marker_1",
                Addr::unchecked("bidder"),
                Addr::unchecked(MOCK_CONTRACT_ADDR)
            )
            .unwrap(),
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
