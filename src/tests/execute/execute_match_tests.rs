#[cfg(test)]
mod execute_match_tests {
    use crate::contract::execute;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{UNHYPHENATED_ASK_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::Uint128;
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn execute_invalid_input_unhyphenated_ids() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base_contract_v3(&mut deps.storage);

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: UNHYPHENATED_ASK_ID.into(),
            bid_id: UNHYPHENATED_BID_ID.into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"ask_id".into()));
                    assert!(fields.contains(&"bid_id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }
}
