#[cfg(test)]
mod get_ask_tests {
    use crate::contract::query;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_query_invalid_id_field;
    use cosmwasm_std::testing::mock_env;
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn query_ask_order_invalid_input_unhyphenated_id() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: UNHYPHENATED_ASK_ID.into(),
            },
        );

        validate_query_invalid_id_field(query_ask_response)
    }
}
