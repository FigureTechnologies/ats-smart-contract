#[cfg(test)]
mod create_bid_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_BID_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coins, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn create_bid_valid_data_unhyphenated() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: UNHYPHENATED_BID_ID.to_string(), // Unhyphenated UUID
            base: "base_1".into(),
            fee: None,
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(250, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        validate_execute_invalid_id_field(create_bid_response)
    }
}
