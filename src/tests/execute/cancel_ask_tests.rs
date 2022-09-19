#[cfg(test)]
mod cancel_ask_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn cancel_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: UNHYPHENATED_ASK_ID.to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        // verify execute cancel ask response
        validate_execute_invalid_id_field(cancel_ask_response)
    }
}
