use crate::common::{Action, Event};
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{
    coin, Addr, BankMsg, Coin, CosmosMsg, DepsMut, Env, Order, QuerierWrapper, Response, Storage,
    Uint128,
};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use provwasm_std::{
    transfer_marker_coins, Marker, MarkerType, ProvenanceMsg, ProvenanceQuerier, ProvenanceQuery,
};
use rust_decimal::prelude::{FromPrimitive, FromStr, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use schemars::JsonSchema;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

pub static NAMESPACE_ORDER_BID: &[u8] = b"bid";

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

pub fn migrate_bid_orders(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    _msg: &MigrateMsg,
    mut response: Response<ProvenanceMsg>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let store = deps.storage;
    let querier = deps.querier;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // bid fees and order events added in 0.16.3, refunds may be necessary for existing orders
    if VersionReq::parse("<0.16.3")?.matches(&current_version) {
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
            let mut existing_bid_order =
                get_bid_storage_read(store).load(&existing_bid_order_id)?;

            response = calculate_migrate_refund(&querier, &env, response, &mut existing_bid_order)
                .unwrap();

            get_bid_storage(store).save(&existing_bid_order_id, &existing_bid_order)?
        }
    }

    Ok(response)
}

fn calculate_migrate_refund(
    querier: &QuerierWrapper<ProvenanceQuery>,
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

pub fn get_bid_storage(storage: &mut dyn Storage) -> Bucket<BidOrderV2> {
    bucket(storage, NAMESPACE_ORDER_BID)
}

pub fn get_bid_storage_read(storage: &dyn Storage) -> ReadonlyBucket<BidOrderV2> {
    bucket_read(storage, NAMESPACE_ORDER_BID)
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use crate::bid_order::BidOrderV2;
    use crate::common::{Action, Event};
    use crate::error::ContractError;
    use cosmwasm_std::{Addr, Coin, Uint128};
    use rust_decimal::prelude::FromStr;
    use rust_decimal::Decimal;

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
