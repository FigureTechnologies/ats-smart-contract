use crate::common::{Action, Event};
use crate::contract_info::require_version;
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Coin, DepsMut, Env, Order, Response, Uint128};
use cw_storage_plus::Map;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

pub const NAMESPACE_ORDER_BID: &str = "bid";

pub const BIDS_V2: Map<&[u8], BidOrderV2> = Map::new(NAMESPACE_ORDER_BID);
pub const BIDS_V3: Map<&[u8], BidOrderV3> = Map::new(NAMESPACE_ORDER_BID);

#[deprecated(since = "0.18.2")]
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

#[allow(deprecated)]
impl BidOrderV2 {
    fn sum_base(&self) -> Uint128 {
        self.events
            .iter()
            .map(|event| match &event.action {
                Action::Fill { base, .. } => base.amount,
                Action::Reject { base, .. } => base.amount,
                _ => Uint128::zero(),
            })
            .sum::<Uint128>()
    }

    fn sum_quote(&self) -> Uint128 {
        self.events
            .iter()
            .map(|event| match &event.action {
                Action::Fill { quote, .. } => quote.amount,
                Action::Refund { quote, .. } => quote.amount,
                Action::Reject { quote, .. } => quote.amount,
            })
            .sum::<Uint128>()
    }

    fn sum_fee(&self) -> Uint128 {
        self.events
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidOrderV3 {
    pub base: Coin,
    pub accumulated_base: Uint128,
    pub accumulated_quote: Uint128,
    pub accumulated_fee: Uint128,
    pub fee: Option<Coin>,
    pub id: String,
    pub owner: Addr,
    pub price: String,
    pub quote: Coin,
}

#[allow(deprecated)]
impl From<BidOrderV2> for BidOrderV3 {
    fn from(old_bid: BidOrderV2) -> Self {
        Self {
            base: old_bid.base.clone(),
            accumulated_base: old_bid.sum_base(),
            accumulated_quote: old_bid.sum_quote(),
            accumulated_fee: old_bid.sum_fee(),
            fee: old_bid.fee,
            id: old_bid.id,
            owner: old_bid.owner,
            price: old_bid.price,
            quote: old_bid.quote,
        }
    }
}

impl BidOrderV3 {
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
        self.base.amount - self.accumulated_base
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
            Some(fee) => fee.amount - self.accumulated_fee,
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
        self.quote.amount - self.accumulated_quote
    }

    /// Update remaining base, fee, and quote amounts based on the given `Action`.
    pub fn update_remaining_amounts(&mut self, action: &Action) -> Result<(), ContractError> {
        match action {
            Action::Fill {
                base,
                fee,
                price: _,
                quote,
            } => {
                // Update base:
                self.accumulated_base = self.accumulated_base.checked_add(base.amount)?;
                // Update fee:
                if let Some(fee) = fee {
                    self.accumulated_fee = self.accumulated_fee.checked_add(fee.amount)?;
                }
                // Update quote:
                self.accumulated_quote = self.accumulated_quote.checked_add(quote.amount)?;
            }
            Action::Refund { fee, quote } => {
                // Update fee:
                if let Some(fee) = fee {
                    self.accumulated_fee = self.accumulated_fee.checked_add(fee.amount)?;
                }
                // Update quote:
                self.accumulated_quote = self.accumulated_quote.checked_add(quote.amount)?;
            }
            Action::Reject { base, fee, quote } => {
                // Update base:
                self.accumulated_base = self.accumulated_base.checked_add(base.amount)?;
                // Update fee:
                if let Some(fee) = fee {
                    self.accumulated_fee = self.accumulated_fee.checked_add(fee.amount)?;
                }
                // Update quote:
                self.accumulated_quote = self.accumulated_quote.checked_add(quote.amount)?;
            }
        }
        Ok(())
    }
}

#[allow(deprecated)]
pub fn migrate_bid_orders(
    deps: DepsMut,
    _env: Env,
    _msg: &MigrateMsg,
    response: Response,
) -> Result<Response, ContractError> {
    let store = deps.storage;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    require_version(">=0.16.2", &current_version)?;

    // Migrate only BidOrderV2 -> BidOrderV3
    if VersionReq::parse(">=0.16.2, <0.19.1")?.matches(&current_version) {
        // get all bid ids for orders in a BidOrderV2 format
        let existing_bid_order_v2_ids: Vec<Vec<u8>> = BIDS_V2
            .range(store, None, None, Order::Ascending)
            .filter_map(|kv_bid| kv_bid.ok().map(|record| record.0))
            .collect();

        // Load the BidOrderV2 items, convert to BidOrderV3, save as BidOrderV3
        for existing_bid_order_v2_id in existing_bid_order_v2_ids {
            let bid_order_v2: BidOrderV2 = BIDS_V2.load(store, &existing_bid_order_v2_id)?;
            let bid_order_v3: BidOrderV3 = bid_order_v2.into();

            BIDS_V3.save(store, &existing_bid_order_v2_id, &bid_order_v3)?
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::{migrate_bid_orders, BidOrderV2, BidOrderV3};
    use crate::bid_order::{BIDS_V2, BIDS_V3};
    use crate::common::{Action, Event};
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1, CRATE_NAME};
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{Addr, Coin, Response, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use rust_decimal::prelude::FromStr;
    use rust_decimal::Decimal;

    #[test]
    pub fn get_functions() -> Result<(), ContractError> {
        let bid_order = BidOrderV3 {
            base: Coin {
                amount: Uint128::new(100),
                denom: "base_1".to_string(),
            },
            accumulated_base: Uint128::new(10 + 10),
            accumulated_quote: Uint128::new(20 + 80 + 100),
            accumulated_fee: Uint128::new(2 + 8),
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

    #[test]
    pub fn bid_migration_fails_if_contract_is_too_old() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        // Contract too old:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.16.1".to_string(), // version too old
            },
        )?;

        let result = {
            let response: Response = Response::new();
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
            )
        };

        match result {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedUpgrade {
                    source_version,
                    target_version,
                } => {
                    assert_eq!(source_version, "0.16.1");
                    assert_eq!(target_version, ">=0.16.2");
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        Ok(())
    }

    #[test]
    pub fn bid_migration_minimum_version_check() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        // Contract minimum version:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.16.2".to_string(), // version is minimum supported
            },
        )?;

        let result = {
            let response: Response = Response::new();
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
            )
        };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Response::new());

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    pub fn migrate_bid_order_v2_to_bid_order_v3() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.16.3".to_string(), // go beyond the minimum version
            },
        )?;

        // Store some v2 bid orders:
        let bid1 = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(8),
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
            id: "bid-1".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "10".to_string(),
            quote: Coin {
                amount: Uint128::new(100),
                denom: "quote_1".to_string(),
            },
        };

        let bid2 = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(16),
                denom: "base_2".to_string(),
            },
            events: vec![
                Event {
                    action: Action::Fill {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(2),
                        }),
                        price: "2".to_string(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(15),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Fill {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(50),
                        },
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(4),
                        }),
                        price: "2".to_string(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(60),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Refund {
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(7),
                        }),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(300),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Refund {
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(15),
                        }),
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(1000),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Reject {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Default::default(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(600),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Reject {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(13),
                        },
                        fee: Default::default(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(198),
                        },
                    },
                    block_info: Default::default(),
                },
            ],
            fee: Some(Coin {
                denom: "base_2".to_string(),
                amount: Uint128::new(5),
            }),
            id: "bid-2".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "15".to_string(),
            quote: Coin {
                amount: Uint128::new(200),
                denom: "quote_2".to_string(),
            },
        };

        // Store:
        BIDS_V2.save(&mut deps.storage, &bid1.id.as_bytes(), &bid1)?;
        BIDS_V2.save(&mut deps.storage, &bid2.id.as_bytes(), &bid2)?;

        // Migrate:
        let response = {
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
            )?
        };

        assert_eq!(response, Response::new());

        // Fetch and verify:
        let bid_1_v3 = BIDS_V3.load(&deps.storage, b"bid-1").unwrap();
        let bid_2_v3 = BIDS_V3.load(&deps.storage, b"bid-2").unwrap();

        assert_eq!(
            BidOrderV3 {
                base: Coin {
                    denom: "base_1".to_owned(),
                    amount: Uint128::new(8)
                },
                accumulated_base: Uint128::new(10 + 10),
                accumulated_quote: Uint128::new(20 + 80 + 100),
                accumulated_fee: Uint128::new(2 + 8),
                fee: None,
                id: "bid-1".to_owned(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_owned(),
                quote: Coin {
                    denom: "quote_1".to_owned(),
                    amount: Uint128::new(100)
                },
            },
            bid_1_v3
        );
        assert_eq!(
            BidOrderV3 {
                base: Coin {
                    denom: "base_2".to_owned(),
                    amount: Uint128::new(16)
                },
                accumulated_base: Uint128::new(10 + 50 + 10 + 13),
                accumulated_quote: Uint128::new(15 + 60 + 300 + 1000 + 600 + 198),
                accumulated_fee: Uint128::new(2 + 4 + 7 + 15),
                fee: Some(Coin {
                    denom: "base_2".to_owned(),
                    amount: Uint128::new(5)
                }),
                id: "bid-2".to_owned(),
                owner: Addr::unchecked("bidder"),
                price: "15".to_owned(),
                quote: Coin {
                    denom: "quote_2".to_owned(),
                    amount: Uint128::new(200)
                },
            },
            bid_2_v3
        );

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    pub fn migrate_contract_v0_19_0_bid_order_v2_to_bid_order_v3() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.19.0".to_string(), // Version with broken conversion
            },
        )?;

        // Store some v2 bid orders:
        // Orders that were unsuccessfully migrated
        let bid1 = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(8),
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
            id: "bid-1".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "10".to_string(),
            quote: Coin {
                amount: Uint128::new(100),
                denom: "quote_1".to_string(),
            },
        };

        let bid2 = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(16),
                denom: "base_2".to_string(),
            },
            events: vec![
                Event {
                    action: Action::Fill {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(2),
                        }),
                        price: "2".to_string(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(15),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Fill {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(50),
                        },
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(4),
                        }),
                        price: "2".to_string(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(60),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Refund {
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(7),
                        }),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(300),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Refund {
                        fee: Some(Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(15),
                        }),
                        quote: Coin {
                            denom: "quote_1".to_string(),
                            amount: Uint128::new(1000),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Reject {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(10),
                        },
                        fee: Default::default(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(600),
                        },
                    },
                    block_info: Default::default(),
                },
                Event {
                    action: Action::Reject {
                        base: Coin {
                            denom: "base_2".to_string(),
                            amount: Uint128::new(13),
                        },
                        fee: Default::default(),
                        quote: Coin {
                            denom: "quote_2".to_string(),
                            amount: Uint128::new(198),
                        },
                    },
                    block_info: Default::default(),
                },
            ],
            fee: Some(Coin {
                denom: "base_2".to_string(),
                amount: Uint128::new(5),
            }),
            id: "bid-2".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "15".to_string(),
            quote: Coin {
                amount: Uint128::new(200),
                denom: "quote_2".to_string(),
            },
        };
        BIDS_V2.save(&mut deps.storage, &bid1.id.as_bytes(), &bid1)?;
        BIDS_V2.save(&mut deps.storage, &bid2.id.as_bytes(), &bid2)?;

        // Orders on latest version
        // Example of order with new version
        let bid3 = BidOrderV3 {
            base: Coin {
                amount: Uint128::new(280000000000),
                denom: "base_2".to_string(),
            },
            accumulated_base: Uint128::new(190000000000),
            accumulated_quote: Uint128::new(4560),
            accumulated_fee: Uint128::new(0),
            fee: Some(Coin {
                denom: "base_2".to_string(),
                amount: Uint128::new(5),
            }),
            id: "bid-3".to_string(),
            owner: Addr::unchecked("bidder"),
            price: "0.000000024".to_string(),
            quote: Coin {
                amount: Uint128::new(6720),
                denom: "quote_2".to_string(),
            },
        };
        BIDS_V3.save(&mut deps.storage, &bid3.id.as_bytes(), &bid3)?;

        // Migrate:
        let response = {
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
            )?
        };

        assert_eq!(response, Response::new());

        // Fetch and verify:
        let bid_1_v3 = BIDS_V3.load(&deps.storage, b"bid-1").unwrap();
        let bid_2_v3 = BIDS_V3.load(&deps.storage, b"bid-2").unwrap();
        let bid_3_v3 = BIDS_V3.load(&deps.storage, b"bid-3").unwrap();

        assert_eq!(
            BidOrderV3 {
                base: Coin {
                    denom: "base_1".to_owned(),
                    amount: Uint128::new(8)
                },
                accumulated_base: Uint128::new(10 + 10),
                accumulated_quote: Uint128::new(20 + 80 + 100),
                accumulated_fee: Uint128::new(2 + 8),
                fee: None,
                id: "bid-1".to_owned(),
                owner: Addr::unchecked("bidder"),
                price: "10".to_owned(),
                quote: Coin {
                    denom: "quote_1".to_owned(),
                    amount: Uint128::new(100)
                },
            },
            bid_1_v3
        );
        assert_eq!(
            BidOrderV3 {
                base: Coin {
                    denom: "base_2".to_owned(),
                    amount: Uint128::new(16)
                },
                accumulated_base: Uint128::new(10 + 50 + 10 + 13),
                accumulated_quote: Uint128::new(15 + 60 + 300 + 1000 + 600 + 198),
                accumulated_fee: Uint128::new(2 + 4 + 7 + 15),
                fee: Some(Coin {
                    denom: "base_2".to_owned(),
                    amount: Uint128::new(5)
                }),
                id: "bid-2".to_owned(),
                owner: Addr::unchecked("bidder"),
                price: "15".to_owned(),
                quote: Coin {
                    denom: "quote_2".to_owned(),
                    amount: Uint128::new(200)
                },
            },
            bid_2_v3
        );

        // Should be the same as before
        assert_eq!(bid3, bid_3_v3);

        Ok(())
    }
}
