#[cfg(test)]
mod get_bid_tests {
    use crate::bid_order::{get_bid_storage, BidOrderV2};
    use crate::contract::query;
    use crate::contract_info::ContractInfoV3;
    use crate::msg::QueryMsg;
    use crate::tests::test_constants::{HYPHENATED_BID_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_utils::setup_test_base;
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{to_binary, Addr, Coin, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn query_bid_order_unhyphenated_id() {
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

        // store valid bid order
        let bid_order = BidOrderV2 {
            base: Coin {
                amount: Uint128::new(100),
                denom: "base_1".into(),
            },
            events: vec![],
            fee: None,
            id: HYPHENATED_BID_ID.into(),
            owner: Addr::unchecked("bidder"),
            price: "2".into(),
            quote: Coin {
                amount: Uint128::new(100),
                denom: "quote_1".into(),
            },
        };

        let mut bid_storage = get_bid_storage(&mut deps.storage);
        if let Err(error) = bid_storage.save(bid_order.id.as_bytes(), &bid_order) {
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
