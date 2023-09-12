use crate::ask_order::ASKS_V1;
use crate::bid_order::BIDS_V3;
use crate::common::{ContractAction, FeeInfo};
use crate::contract_info::{get_contract_info, modify_contract_info};
use crate::error::ContractError;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response};
use provwasm_std::{ProvenanceMsg, ProvenanceQuery};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::str::FromStr;

pub fn modify_contract(
    deps: DepsMut<ProvenanceQuery>,
    _env: Env,
    info: &MessageInfo,
    approvers: Option<Vec<String>>,
    executors: Option<Vec<String>>,
    ask_fee_rate: Option<String>,
    ask_fee_account: Option<String>,
    bid_fee_rate: Option<String>,
    bid_fee_account: Option<String>,
    ask_required_attributes: Option<Vec<String>>,
    bid_required_attributes: Option<Vec<String>>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    if !contract_info.executors.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    let contains_ask = !ASKS_V1.is_empty(deps.storage);
    check_required_attributes(
        contains_ask.to_owned(),
        ask_required_attributes.to_owned(),
        "ask_required_attributes".to_string(),
    )?;
    check_fee_rate(
        contains_ask.to_owned(),
        contract_info.ask_fee_info.to_owned(),
        ask_fee_rate.to_owned(),
        ask_fee_account.to_owned(),
        "ask_fee".to_string(),
    )?;

    let contains_bid = !BIDS_V3.is_empty(deps.storage);
    check_required_attributes(
        contains_bid.to_owned(),
        bid_required_attributes.to_owned(),
        "bid_required_attributes".to_string(),
    )?;
    check_fee_rate(
        contains_bid.to_owned(),
        contract_info.bid_fee_info.to_owned(),
        bid_fee_rate.to_owned(),
        bid_fee_account.to_owned(),
        "bid_fee".to_string(),
    )?;

    if contains_ask || contains_bid {
        match &approvers {
            None => {}
            Some(approvers) => {
                let current_approvers: HashSet<String> = contract_info
                    .approvers
                    .into_iter()
                    .map(|item| item.into_string())
                    .collect();
                let new_approvers: HashSet<String> = approvers.clone().into_iter().collect();
                if !current_approvers.is_subset(&new_approvers) {
                    return Err(ContractError::InvalidFields {
                        fields: vec!["approvers".to_string()],
                    });
                }
            }
        }
    }

    modify_contract_info(
        deps,
        approvers,
        executors,
        ask_fee_rate,
        ask_fee_account,
        bid_fee_rate,
        bid_fee_account,
        ask_required_attributes,
        bid_required_attributes,
    )?;

    let response =
        Response::new().add_attribute("action", ContractAction::ModifyContract.to_string());

    Ok(response)
}

fn check_required_attributes(
    contains_attribute_side_order: bool,
    new_required_attributes: Option<Vec<String>>,
    error_field_name: String,
) -> Result<(), ContractError> {
    if contains_attribute_side_order {
        match &new_required_attributes {
            None => {}
            Some(_) => {
                return Err(ContractError::InvalidFields {
                    fields: vec![error_field_name],
                });
            }
        }
    }

    Ok(())
}

fn check_fee_rate(
    contains_fee_side_order: bool,
    current_fee_info: Option<FeeInfo>,
    new_fee_rate: Option<String>,
    new_fee_account: Option<String>,
    error_field_name: String,
) -> Result<(), ContractError> {
    if contains_fee_side_order {
        match (&new_fee_rate, &new_fee_account) {
            (None, None) => {}
            (Some(new_fee_rate_str), _) => {
                let invalid_change = match current_fee_info {
                    Some(current_fee) => {
                        // Currently there is a fee, make sure the rate is not changing

                        let current_fee_rate_dec = Decimal::from_str(&current_fee.rate).unwrap();
                        let new_fee_rate_dec = Decimal::from_str(&new_fee_rate_str).unwrap();
                        current_fee_rate_dec.ne(&new_fee_rate_dec)
                    }
                    None => {
                        // Currently there is no fee

                        // Error since trying to set a fee with an existing ask order
                        true
                    }
                };
                if invalid_change {
                    return Err(ContractError::InvalidFields {
                        fields: vec![error_field_name],
                    });
                }
            }
            (_, _) => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod modify_contract_check_tests {
    use crate::common::FeeInfo;
    use crate::error::ContractError;
    use crate::execute::modify_contract::check_fee_rate;
    use cosmwasm_std::Addr;

    #[test]
    fn check_fee_rate_contains_order_and_only_account_change_return_ok() {
        let result = check_fee_rate(
            true,
            Some(FeeInfo {
                rate: "0.010".to_string(), // String is different, but Decimal is same
                account: Addr::unchecked("old_account"),
            }),
            Some("0.01".to_string()), // String is different, but Decimal is same
            Some("new_account".to_string()),
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => {}
            Err(error) => {
                panic!("unexpected error: {:?}", error)
            }
        }
    }

    #[test]
    fn check_fee_rate_contains_order_and_rate_change_return_err() {
        let result = check_fee_rate(
            true,
            Some(FeeInfo {
                rate: "0.02".to_string(),
                account: Addr::unchecked("old_account"),
            }),
            Some("0.01".to_string()),
            Some("new_account".to_string()),
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["test_fee"]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn check_fee_rate_contains_order_and_rate_change_from_none_return_err() {
        let result = check_fee_rate(
            true,
            None,
            Some("0.01".to_string()),
            Some("new_account".to_string()),
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["test_fee"]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn check_fee_rate_contains_order_and_only_rate_change_return_err() {
        let result = check_fee_rate(
            true,
            Some(FeeInfo {
                rate: "0.02".to_string(),
                account: Addr::unchecked("old_account"),
            }),
            Some("0.01".to_string()),
            None,
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["test_fee"]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn check_fee_rate_contains_order_and_only_account_set_return_ok() {
        // This should not happen because the msg.validate() checks that
        //  the fee_rate and fee_account are modified together
        let result = check_fee_rate(
            true,
            None,
            None,
            Some("new_account".to_string()),
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => {}
            Err(error) => {
                panic!("unexpected error: {:?}", error)
            }
        }
    }

    #[test]
    fn check_fee_rate_no_order_return_ok() {
        let result = check_fee_rate(
            false,
            None,
            Some("0.01".to_string()),
            Some("new_account".to_string()),
            "test_fee".to_string(),
        );
        match result {
            Ok(_) => {}
            Err(error) => {
                panic!("unexpected error: {:?}", error)
            }
        }
    }
}
