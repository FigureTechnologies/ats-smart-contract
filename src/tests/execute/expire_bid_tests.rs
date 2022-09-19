#[cfg(test)]
mod expire_bid_tests {
    use crate::contract::execute;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_BID_ID;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn expire_bid_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: UNHYPHENATED_BID_ID.to_string(),
        };

        let expire_bid_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

        validate_execute_invalid_id_field(expire_bid_response)
    }
}
