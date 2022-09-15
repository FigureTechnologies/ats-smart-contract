#[cfg(test)]
mod execute_match_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderV1};
    use crate::bid_order::{get_bid_storage_read, BidOrderV2};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{
        HYPHENATED_ASK_ID, HYPHENATED_BID_ID, UNHYPHENATED_ASK_ID, UNHYPHENATED_BID_ID,
    };
    use crate::tests::test_utils::{setup_test_base, store_test_ask, store_test_bid};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, coins, Addr, BankMsg, Coin, CosmosMsg, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn execute_valid_data_unhyphenated_id() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: HYPHENATED_ASK_ID.into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: HYPHENATED_BID_ID.into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: UNHYPHENATED_ASK_ID.into(),
            bid_id: UNHYPHENATED_BID_ID.into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 9);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", HYPHENATED_ASK_ID)
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", HYPHENATED_BID_ID)
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage.load(HYPHENATED_ASK_ID.as_bytes()).is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage.load(HYPHENATED_BID_ID.as_bytes()).is_err());
    }
}
