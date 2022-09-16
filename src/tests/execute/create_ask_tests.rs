#[cfg(test)]
mod create_ask_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn create_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: UNHYPHENATED_ASK_ID.into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128::new(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_1"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info,
            create_ask_msg.clone(),
        );

        // verify create ask response
        validate_execute_invalid_id_field(create_ask_response)
    }
}
