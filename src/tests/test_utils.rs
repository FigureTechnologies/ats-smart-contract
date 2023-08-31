use crate::error::ContractError;
use cosmwasm_std::Response;

pub fn validate_execute_invalid_id_field(execute_response: Result<Response, ContractError>) {
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
