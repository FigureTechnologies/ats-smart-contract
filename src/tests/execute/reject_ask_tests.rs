#[cfg(test)]
mod reject_ask_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn reject_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: UNHYPHENATED_ASK_ID.to_string(),
            size: None,
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        validate_execute_invalid_id_field(reject_ask_response)
    }
}
