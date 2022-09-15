#[cfg(test)]
mod get_ask_tests {
    use crate::ask_order::{get_ask_storage, get_ask_storage_read, AskOrderClass, AskOrderV1};
    use crate::contract::query;
    use crate::contract_info::ContractInfoV3;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_utils::setup_test_base;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{to_binary, Addr, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn query_ask_order_unhyphenated_id() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // store valid ask order
        let ask_order = AskOrderV1 {
            base: "base_1".into(),
            class: AskOrderClass::Basic,
            id: HYPHENATED_ASK_ID.into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(200),
        };

        let mut ask_storage = get_ask_storage(&mut deps.storage);
        if let Err(error) = ask_storage.save(HYPHENATED_ASK_ID.as_bytes(), &ask_order) {
            panic!("unexpected error: {:?}", error)
        };

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage.load(HYPHENATED_ASK_ID.as_bytes()).is_ok());

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: UNHYPHENATED_ASK_ID.into(),
            },
        );

        assert_eq!(query_ask_response, to_binary(&ask_order));
    }
}
