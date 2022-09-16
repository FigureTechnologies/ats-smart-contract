#[cfg(test)]
mod approve_ask_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coin, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn approve_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: UNHYPHENATED_ASK_ID.into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // verify execute approve ask response
        validate_execute_invalid_id_field(approve_ask_response)
    }
}
