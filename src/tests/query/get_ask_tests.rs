#[cfg(test)]
mod get_ask_tests {
    use crate::ask_order::{AskOrderClass, AskOrderV1, ASKS_V1};
    use crate::contract::query;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{to_binary, Addr, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;

    #[test]
    fn query_ask_order_where_order_exists_then_return_ask_order() {
        // setup
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

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

        if let Err(error) =
            ASKS_V1.save(&mut deps.storage, HYPHENATED_ASK_ID.as_bytes(), &ask_order)
        {
            panic!("unexpected error: {:?}", error)
        };

        // verify ask order still exists
        assert!(ASKS_V1
            .load(&deps.storage, HYPHENATED_ASK_ID.as_bytes())
            .is_ok());

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: HYPHENATED_ASK_ID.into(),
            },
        );

        assert_eq!(query_ask_response, to_binary(&ask_order));
    }

    #[test]
    fn query_ask_order_legacy_unhyphenated_id_then_returns_ask_order() {
        // setup
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // store old supported ask order
        let ask_order = AskOrderV1 {
            base: "base_1".into(),
            class: AskOrderClass::Basic,
            id: UNHYPHENATED_ASK_ID.into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(200),
        };
        if let Err(error) = ASKS_V1.save(
            &mut deps.storage,
            UNHYPHENATED_ASK_ID.as_bytes(),
            &ask_order,
        ) {
            panic!("unexpected error: {:?}", error)
        };

        // verify ask order still exists
        assert!(ASKS_V1
            .load(&deps.storage, UNHYPHENATED_ASK_ID.as_bytes())
            .is_ok());

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: UNHYPHENATED_ASK_ID.into(),
            },
        );

        assert_eq!(query_ask_response, to_binary(&ask_order))
    }
}
