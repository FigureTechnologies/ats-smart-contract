#[cfg(test)]
mod get_bid_tests {
    use crate::bid_order::{BidOrderV3, BIDS_V3};
    use crate::contract::query;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::{HYPHENATED_BID_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn query_bid_order_where_order_exists_then_return_bid_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // store valid bid order
        let bid_order = BidOrderV3 {
            base: Coin {
                amount: Uint128::new(100),
                denom: "base_1".into(),
            },
            accumulated_base: Uint128::zero(),
            accumulated_quote: Uint128::zero(),
            accumulated_fee: Uint128::zero(),
            fee: None,
            id: HYPHENATED_BID_ID.into(),
            owner: Addr::unchecked("bidder"),
            price: "2".into(),
            quote: Coin {
                amount: Uint128::new(100),
                denom: "quote_1".into(),
            },
        };

        if let Err(error) = BIDS_V3.save(&mut deps.storage, bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };

        // query for bid order
        let query_bid_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBid {
                id: HYPHENATED_BID_ID.into(),
            },
        );

        assert_eq!(query_bid_response, to_binary(&bid_order));
    }

    #[test]
    fn query_bid_order_legacy_unhyphenated_id_returns_bid_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // store legacy unhyphenated bid order
        let bid_order = BidOrderV3 {
            base: Coin {
                amount: Uint128::new(100),
                denom: "base_1".into(),
            },
            accumulated_base: Uint128::zero(),
            accumulated_quote: Uint128::zero(),
            accumulated_fee: Uint128::zero(),
            fee: None,
            id: UNHYPHENATED_BID_ID.into(),
            owner: Addr::unchecked("bidder"),
            price: "2".into(),
            quote: Coin {
                amount: Uint128::new(100),
                denom: "quote_1".into(),
            },
        };

        if let Err(error) = BIDS_V3.save(&mut deps.storage, bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };

        // query for bid order
        let query_bid_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBid {
                id: UNHYPHENATED_BID_ID.into(),
            },
        );

        assert_eq!(query_bid_response, to_binary(&bid_order));
    }
}
