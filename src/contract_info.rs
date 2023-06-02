use cosmwasm_std::{Addr, DepsMut, Storage, Uint128};
use cw_storage_plus::Item;
use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::FeeInfo;
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use provwasm_std::ProvenanceQuery;
use semver::{Version, VersionReq};

const CONTRACT_INFO_NAMESPACE: &str = "contract_info";

const CONTRACT_INFO_V3: Item<ContractInfoV3> = Item::new(CONTRACT_INFO_NAMESPACE);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfoV3 {
    pub name: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub approvers: Vec<Addr>,
    pub executors: Vec<Addr>,
    pub ask_fee_info: Option<FeeInfo>,
    pub bid_fee_info: Option<FeeInfo>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
    pub price_precision: Uint128,
    pub size_increment: Uint128,
}

/// Enforces the specified contract version requirement.
pub fn require_version(
    version_requirement: &str,
    current_version: &Version,
) -> Result<(), ContractError> {
    if !VersionReq::parse(version_requirement)?.matches(&current_version) {
        return Err(ContractError::UnsupportedUpgrade {
            source_version: current_version.to_string(),
            target_version: version_requirement.to_owned(),
        });
    }
    Ok(())
}

pub fn set_contract_info(
    store: &mut dyn Storage,
    contract_info: &ContractInfoV3,
) -> Result<(), ContractError> {
    CONTRACT_INFO_V3
        .save(store, contract_info)
        .map_err(ContractError::Std)
}

pub fn get_contract_info(store: &dyn Storage) -> Result<ContractInfoV3, ContractError> {
    CONTRACT_INFO_V3.load(store).map_err(ContractError::Std)
}

pub fn migrate_contract_info(
    deps: DepsMut<ProvenanceQuery>,
    msg: &MigrateMsg,
) -> Result<ContractInfoV3, ContractError> {
    let store = deps.storage;
    let api = deps.api;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // Enforce a minimum version that can be migrated to - this was put in place
    // after the cleanup of older migration code:
    require_version(">=0.16.2", &current_version)?;

    let mut contract_info = get_contract_info(store)?;
    match &msg.approvers {
        None => {}
        Some(approvers) => {
            // Validate and convert approvers to addresses
            let mut new_approvers: Vec<Addr> = Vec::new();
            for approver_str in approvers {
                let address = api.addr_validate(approver_str)?;
                new_approvers.push(address);
            }

            contract_info.approvers = new_approvers;
        }
    }

    match (&msg.ask_fee_account, &msg.ask_fee_rate) {
        (Some(account), Some(rate)) => {
            contract_info.ask_fee_info = match (account.as_str(), rate.as_str()) {
                ("", "") => None,
                (_, _) => {
                    Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                        fields: vec![String::from("ask_fee_rate")],
                    })?;

                    Some(FeeInfo {
                        account: api.addr_validate(account)?,
                        rate: rate.to_string(),
                    })
                }
            }
        }
        (_, _) => (),
    };

    match (&msg.bid_fee_account, &msg.bid_fee_rate) {
        (Some(account), Some(rate)) => {
            contract_info.bid_fee_info = match (account.as_str(), rate.as_str()) {
                ("", "") => None,
                (_, _) => {
                    Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                        fields: vec![String::from("bid_fee_rate")],
                    })?;

                    Some(FeeInfo {
                        account: api.addr_validate(account)?,
                        rate: rate.to_string(),
                    })
                }
            }
        }
        (_, _) => (),
    };

    match &msg.ask_required_attributes {
        None => {}
        Some(ask_required_attributes) => {
            contract_info.ask_required_attributes = ask_required_attributes.clone();
        }
    }

    match &msg.bid_required_attributes {
        None => {}
        Some(bid_required_attributes) => {
            contract_info.bid_required_attributes = bid_required_attributes.clone();
        }
    }

    set_contract_info(store, &contract_info)?;

    get_contract_info(store)
}

pub fn modify_contract_info(
    deps: DepsMut<ProvenanceQuery>,
    approvers: Option<Vec<String>>,
    executors: Option<Vec<String>>,
    ask_fee_rate: Option<String>,
    ask_fee_account: Option<String>,
    bid_fee_rate: Option<String>,
    bid_fee_account: Option<String>,
    ask_required_attributes: Option<Vec<String>>,
    bid_required_attributes: Option<Vec<String>>,
) -> Result<ContractInfoV3, ContractError> {
    let store = deps.storage;
    let api = deps.api;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    if VersionReq::parse("<0.16.2")?.matches(&current_version) {
        return Err(ContractError::UnsupportedUpgrade {
            source_version: "<0.16.2".to_string(),
            target_version: ">=0.16.2".to_string(),
        });
    }

    let mut contract_info = get_contract_info(store)?;
    match &approvers {
        None => {}
        Some(approvers) => {
            let mut new_approvers: Vec<Addr> = Vec::new();
            for approver_str in approvers {
                let address = api.addr_validate(approver_str)?;
                new_approvers.push(address);
            }

            contract_info.approvers = new_approvers;
        }
    }

    match &executors {
        None => {}
        Some(executors) => {
            let mut new_executors: Vec<Addr> = Vec::new();
            for executor_str in executors {
                let address = api.addr_validate(executor_str)?;
                new_executors.push(address);
            }

            contract_info.executors = new_executors;
        }
    }

    match (&ask_fee_account, &ask_fee_rate) {
        (Some(account), Some(rate)) => {
            contract_info.ask_fee_info = match (account.as_str(), rate.as_str()) {
                ("", "") => None,
                (_, _) => {
                    Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                        fields: vec![String::from("ask_fee_rate")],
                    })?;

                    Some(FeeInfo {
                        account: api.addr_validate(account)?,
                        rate: rate.to_string(),
                    })
                }
            }
        }
        (_, _) => (),
    };

    match (&bid_fee_account, &bid_fee_rate) {
        (Some(account), Some(rate)) => {
            contract_info.bid_fee_info = match (account.as_str(), rate.as_str()) {
                ("", "") => None,
                (_, _) => {
                    Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                        fields: vec![String::from("bid_fee_rate")],
                    })?;

                    Some(FeeInfo {
                        account: api.addr_validate(account)?,
                        rate: rate.to_string(),
                    })
                }
            }
        }
        (_, _) => (),
    };

    match &ask_required_attributes {
        None => {}
        Some(ask_required_attributes) => {
            contract_info.ask_required_attributes = ask_required_attributes.clone();
        }
    }

    match &bid_required_attributes {
        None => {}
        Some(bid_required_attributes) => {
            contract_info.bid_required_attributes = bid_required_attributes.clone();
        }
    }

    set_contract_info(store, &contract_info)?;

    get_contract_info(store)
}

#[cfg(test)]
mod tests {
    use provwasm_mocks::mock_dependencies;

    #[allow(deprecated)]
    use super::{
        get_contract_info, migrate_contract_info, require_version, set_contract_info,
        ContractInfoV3, Version,
    };
    use crate::common::FeeInfo;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{get_version_info, set_version_info, VersionInfoV1};
    use cosmwasm_std::{Addr, Uint128};

    #[test]
    pub fn set_contract_info_with_valid_data() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        set_contract_info(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quo_base_1".into(), "quo_base_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("ask_fee_acct"),
                    rate: "0.00".to_string(),
                }),
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_acct"),
                    rate: "0.02".to_string(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                price_precision: Uint128::new(3),
                size_increment: Uint128::new(1000),
            },
        )?;

        let contract_info = get_contract_info(&deps.storage)?;

        assert_eq!(contract_info.name, "contract_name");
        assert_eq!(contract_info.bind_name, "contract_bind_name");
        assert_eq!(contract_info.base_denom, "base_denom");
        assert_eq!(
            contract_info.convertible_base_denoms,
            vec!["con_base_1", "con_base_2"]
        );
        assert_eq!(
            contract_info.supported_quote_denoms,
            vec!["quo_base_1", "quo_base_2"]
        );
        assert_eq!(
            contract_info.approvers,
            vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")]
        );
        assert_eq!(
            contract_info.executors,
            vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")]
        );
        assert_eq!(
            contract_info.ask_fee_info,
            Some(FeeInfo {
                account: Addr::unchecked("ask_fee_acct"),
                rate: "0.00".to_string(),
            })
        );
        assert_eq!(
            contract_info.bid_fee_info,
            Some(FeeInfo {
                account: Addr::unchecked("bid_fee_acct"),
                rate: "0.02".to_string(),
            })
        );
        assert_eq!(
            contract_info.ask_required_attributes,
            vec!["ask_tag_1", "ask_tag_2"]
        );
        assert_eq!(
            contract_info.bid_required_attributes,
            vec!["ask_tag_1", "ask_tag_2"]
        );
        assert_eq!(contract_info.price_precision, Uint128::new(3));
        assert_eq!(contract_info.size_increment, Uint128::new(1000));

        Ok(())
    }

    #[test]
    fn old_contract_versions_are_not_supported() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.1".to_string(), // too old
            },
        )?;

        let store = deps.storage;
        let version_info = get_version_info(&store)?;
        let current_version = Version::parse(&version_info.version)?;

        let result = require_version(">=0.16.2", &current_version);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn newer_contract_versions_are_supported() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(), // 0.16.2 onward is supported
            },
        )?;

        let store = deps.storage;
        let version_info = get_version_info(&store)?;
        let current_version = Version::parse(&version_info.version)?;

        let result = require_version(">=0.16.2", &current_version);
        assert!(result.is_ok());

        Ok(())
    }

    #[test]
    fn migrate_without_data_is_unchanged() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        )?;

        set_contract_info(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("ask_fee_account"),
                    rate: "0.01".into(),
                }),
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.02".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        )?;

        // migrate without fees
        migrate_contract_info(
            deps.as_mut(),
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

        // verify contract_info is unchanged
        let contract_info = get_contract_info(&deps.storage).unwrap();
        let expected_contract_info = ContractInfoV3 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            ask_fee_info: Some(FeeInfo {
                account: Addr::unchecked("ask_fee_account"),
                rate: "0.01".into(),
            }),
            bid_fee_info: Some(FeeInfo {
                account: Addr::unchecked("bid_fee_account"),
                rate: "0.02".into(),
            }),
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }

    #[test]
    fn migrate_with_data_is_changed() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        )?;

        set_contract_info(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("ask_fee_account"),
                    rate: "0.01".into(),
                }),
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.02".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        )?;

        // migrate with new fees
        migrate_contract_info(
            deps.as_mut(),
            &MigrateMsg {
                approvers: Some(vec!["approver_3".into(), "approver_4".into()]),
                ask_fee_rate: Some("0.03".into()),
                ask_fee_account: Some("new_ask_fee_account".into()),
                bid_fee_rate: Some("0.04".into()),
                bid_fee_account: Some("new_bid_fee_account".into()),
                ask_required_attributes: Some(vec!["ask_tag_3".into(), "ask_tag_4".into()]),
                bid_required_attributes: Some(vec!["bid_tag_3".into(), "bid_tag_4".into()]),
            },
        )?;

        // verify contract_info updated
        let contract_info = get_contract_info(&deps.storage).unwrap();
        let expected_contract_info = ContractInfoV3 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked("approver_3"), Addr::unchecked("approver_4")],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            ask_fee_info: Some(FeeInfo {
                account: Addr::unchecked("new_ask_fee_account"),
                rate: "0.03".into(),
            }),
            bid_fee_info: Some(FeeInfo {
                account: Addr::unchecked("new_bid_fee_account"),
                rate: "0.04".into(),
            }),
            ask_required_attributes: vec!["ask_tag_3".into(), "ask_tag_4".into()],
            bid_required_attributes: vec!["bid_tag_3".into(), "bid_tag_4".into()],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }
}
