use crate::error::ContractError;
use cosmwasm_std::{Addr, Empty, QuerierWrapper, StdError, StdResult, Uint128};
use provwasm_std::types::cosmos::base::v1beta1::Coin;
use provwasm_std::types::provenance::attribute::v1::{Attribute, AttributeQuerier};
use provwasm_std::types::provenance::marker::v1::{
    MarkerAccount, MarkerQuerier, MsgTransferRequest,
};
use rust_decimal::prelude::Zero;
use rust_decimal::Decimal;
use std::convert::TryFrom;
use uuid::Uuid;

pub fn is_restricted_marker(querier: &QuerierWrapper, denom: String) -> bool {
    matches!(
        get_marker(denom.clone(), &MarkerQuerier::new(&querier)),
        Ok(MarkerAccount {
            marker_type: 2, // 2 is Restricted
            ..
        })
    )
}

fn get_marker(id: String, querier: &MarkerQuerier<Empty>) -> StdResult<MarkerAccount> {
    let response = querier.marker(id)?;
    if let Some(marker) = response.marker {
        return if let Ok(account) = MarkerAccount::try_from(marker) {
            Ok(account)
        } else {
            Err(StdError::generic_err("unable to type-cast marker account"))
        };
    } else {
        Err(StdError::generic_err("no marker found for id"))
    }
}

// TODO: temporary function, need to find alternative function
pub fn get_attributes(
    account: String,
    querier: &AttributeQuerier<Empty>,
) -> StdResult<Vec<Attribute>> {
    return match querier.attributes(account, None) {
        Ok(response) => Ok(response.attributes),
        Err(error) => Err(error),
    };
}

pub fn transfer_marker_coins<S: Into<String>, H: Into<Addr>>(
    amount: u128,
    denom: S,
    to: H,
    from: H,
    contract_address: H,
) -> StdResult<MsgTransferRequest> {
    if amount == 0 {
        return Err(StdError::generic_err("transfer amount must be > 0"));
    }

    let coin = Coin {
        denom: denom.into().to_string(),
        amount: amount.to_string(),
    };

    let request = MsgTransferRequest {
        amount: Some(coin),
        administrator: contract_address.into().to_string(),
        from_address: from.into().to_string(),
        to_address: to.into().to_string(),
    };
    Ok(request)
}

pub fn is_invalid_price_precision(price: Decimal, price_precision: Uint128) -> bool {
    price
        .checked_mul(Decimal::from(10u128.pow(price_precision.u128() as u32)))
        .ok_or(ContractError::TotalOverflow)
        .unwrap()
        .fract()
        .ne(&Decimal::zero())
}

fn to_hyphenated_uuid_str(uuid: String) -> Result<String, ContractError> {
    Ok(Uuid::parse_str(uuid.as_str())
        .map_err(ContractError::UuidError)?
        .hyphenated()
        .to_string())
}

pub fn is_hyphenated_uuid_str(uuid: &String) -> bool {
    let hyphenated_uuid_str_result = to_hyphenated_uuid_str(uuid.to_owned());
    if hyphenated_uuid_str_result.is_err() {
        return false;
    }
    if uuid.ne(&hyphenated_uuid_str_result.unwrap()) {
        return false;
    }
    true
}

#[cfg(test)]
mod util_tests {
    use crate::util::{is_hyphenated_uuid_str, to_hyphenated_uuid_str};
    const UUID_HYPHENATED: &str = "093231fc-e4b3-4fbc-a441-838787f16933";
    const UUID_NOT_HYPHENATED: &str = "093231fce4b34fbca441838787f16933";

    #[test]
    fn to_hyphenated_uuid_not_valid_uuid_input_then_return_err() {
        let result = to_hyphenated_uuid_str("INVALID_STR".to_string());

        match result {
            Ok(result_str) => panic!("Expected error: {:?}", result_str),
            _error => {}
        }
    }

    #[test]
    fn to_hyphenated_uuid_input_is_hyphenated_then_return_hyphenated_str() {
        let result = to_hyphenated_uuid_str(UUID_HYPHENATED.to_string());

        match result {
            Ok(result_str) => {
                assert_eq!(result_str, UUID_HYPHENATED)
            }
            error => panic!("Unexpected error: {:?}", error),
        }
    }

    #[test]
    fn to_hyphenated_uuid_input_not_hyphenated_then_return_hyphenated_str() {
        let result = to_hyphenated_uuid_str(UUID_NOT_HYPHENATED.to_string());

        match result {
            Ok(result_str) => {
                assert_eq!(result_str, UUID_HYPHENATED)
            }
            error => panic!("Unexpected error: {:?}", error),
        }
    }

    #[test]
    fn is_hyphenated_uuid_input_not_uuid_then_return_false() {
        let result = is_hyphenated_uuid_str(&"BAD_INPUT".to_string());

        match result {
            true => panic!("Expected false"),
            false => {}
        }
    }

    #[test]
    fn is_hyphenated_uuid_input_not_hyphenated_then_return_false() {
        let result = is_hyphenated_uuid_str(&UUID_NOT_HYPHENATED.to_string());

        match result {
            true => panic!("Expected false"),
            false => {}
        }
    }

    #[test]
    fn is_hyphenated_uuid_input_is_hyphenated_then_return_true() {
        let result = is_hyphenated_uuid_str(&UUID_HYPHENATED.to_string());

        match result {
            true => {}
            false => panic!("Expected true"),
        }
    }
}
