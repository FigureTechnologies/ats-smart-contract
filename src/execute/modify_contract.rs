use crate::ask_order::ASKS_V1;
use crate::bid_order::BIDS_V3;
use crate::common::ContractAction;
use crate::contract_info::{get_contract_info, modify_contract_info};
use crate::error::ContractError;
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response};
use provwasm_std::{ProvenanceMsg, ProvenanceQuery};
use std::collections::HashSet;

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
    match &ask_required_attributes {
        None => (),
        Some(_) => {
            if contains_ask {
                return Err(ContractError::InvalidFields {
                    fields: vec!["ask_required_attributes".to_string()],
                });
            }
        }
    }

    let contains_bid = !BIDS_V3.is_empty(deps.storage);
    if contains_bid {
        match &bid_required_attributes {
            None => {}
            Some(_) => {
                return Err(ContractError::InvalidFields {
                    fields: vec!["bid_required_attributes".to_string()],
                });
            }
        }
        match (&bid_fee_rate, &bid_fee_account) {
            (None, None) => {}
            (_, _) => {
                return Err(ContractError::InvalidFields {
                    fields: vec!["bid_fee".to_string()],
                });
            }
        }
    }

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
