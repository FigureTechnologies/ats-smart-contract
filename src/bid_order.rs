use crate::common::{Action, Event};
use crate::contract_info::require_version;
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Coin, DepsMut, Env, Response, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use provwasm_std::{ProvenanceMsg, ProvenanceQuery};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use schemars::JsonSchema;
use semver::Version;
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
    _env: Env,
    _msg: &MigrateMsg,
    response: Response<ProvenanceMsg>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let store = deps.storage;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    require_version(">=0.16.2", &current_version)?;

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
    use super::migrate_bid_orders;
    use crate::bid_order::BidOrderV2;
    use crate::common::{Action, Event};
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1, CRATE_NAME};
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{Addr, Coin, Response, Uint128};
    use provwasm_mocks::mock_dependencies;
    use provwasm_std::ProvenanceMsg;
    use rust_decimal::prelude::FromStr;
    use rust_decimal::Decimal;

    #[test]
    pub fn bid_migration_version_check() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_dependencies(&[]);

        // Contract too old:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.16.1".to_string(), // version too old
            },
        )?;

        let result = {
            let response: Response<ProvenanceMsg> = Response::new();
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
        assert!(result.is_err());

        // Contract minimum version:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.16.2".to_string(), // version too old
            },
        )?;

        let result = {
            let response: Response<ProvenanceMsg> = Response::new();
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
