use crate::error::ContractError;
use cosmwasm_std::StdError::GenericErr;
use cosmwasm_std::{Binary, Response, StdResult};
use provwasm_std::ProvenanceMsg;

pub fn validate_execute_invalid_id_field(
    execute_response: Result<Response<ProvenanceMsg>, ContractError>,
) {
    match execute_response {
        Ok(_) => panic!("expected error, but ok"),
        Err(error) => match error {
            ContractError::InvalidFields { fields } => {
                assert!(fields.contains(&"id".into()));
            }
            error => panic!("unexpected error: {:?}", error),
        },
    }
}

pub fn validate_query_invalid_id_field(query_response: StdResult<Binary>) {
    match query_response {
        Ok(_) => panic!("expected error, but ok"),
        Err(error) => match error {
            GenericErr { msg } => {
                assert_eq!(msg, "Invalid fields: [\"id\"]")
            }
            error => panic!("unexpected error: {:?}", error),
        },
    }
}
