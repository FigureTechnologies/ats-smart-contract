use crate::error::ContractError;
use uuid::Uuid;

fn to_hyphenated_uuid_str(uuid: String) -> Result<String, ContractError> {
    Ok(Uuid::parse_str(uuid.as_str())
        .map_err(ContractError::UuidError)?
        .to_hyphenated()
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
