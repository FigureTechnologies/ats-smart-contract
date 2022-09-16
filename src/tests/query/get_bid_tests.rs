#[cfg(test)]
mod get_bid_tests {
    use crate::contract::query;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::UNHYPHENATED_BID_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_query_invalid_id_field;
    use cosmwasm_std::testing::mock_env;
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn query_bid_order_invalid_input_unhyphenated_id() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // query for bid order
        let query_bid_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBid {
                id: UNHYPHENATED_BID_ID.into(),
            },
        );

        validate_query_invalid_id_field(query_bid_response)
    }
}
