#[cfg(test)]
mod execute_match_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderStatus, AskOrderV1};
    use crate::bid_order::{get_bid_storage_read, BidOrderV3};
    use crate::common::FeeInfo;
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{UNHYPHENATED_ASK_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_setup_utils::{
        setup_test_base, setup_test_base_contract_v3, store_test_ask, store_test_bid,
    };
    use crate::util::{transfer_marker_coins};
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{
        attr, coin, coins, from_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Storage, Uint128,
    };
    use provwasm_mocks::{mock_provenance_dependencies, mock_provenance_dependencies_with_custom_querier, MockProvenanceQuerier};
    use provwasm_std::types::provenance::marker::v1::MarkerAccount;

    pub fn setup_custom_test_base_contract_v3(
        storage: &mut dyn Storage,
        ask_fee: Option<FeeInfo>,
        bid_fee: Option<FeeInfo>,
        precision: i32,
        size_increment: i32,
    ) {
        setup_test_base(
            storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: ask_fee,
                bid_fee_info: bid_fee,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(precision as u128),
                size_increment: Uint128::new(size_increment as u128),
            },
        );
    }

    #[test]
    fn execute_quote_denom_mismatch_returns_err() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_2".into(), // not equal to "quote_1"
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
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
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedQuoteDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }

    #[test]
    fn execute_invalid_input_unhyphenated_ids() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_test_base_contract_v3(&mut deps.storage);

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
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"ask_id".into()));
                    assert!(fields.contains(&"bid_id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn execute_valid_data() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_test_base_contract_v3(&mut deps.storage);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
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
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
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
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_ask_fees_round_down() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_custom_test_base_contract_v3(
            &mut deps.storage,
            Some(FeeInfo {
                account: Addr::unchecked("ask_fee_account"),
                rate: "0.01".into(),
            }),
            None,
            0,
            1,
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "1"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "ask_fee_account".into(),
                        amount: coins(1, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(148, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_ask_fees_round_up() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_custom_test_base_contract_v3(
            &mut deps.storage,
            Some(FeeInfo {
                account: Addr::unchecked("ask_fee_account"),
                rate: "0.01".into(),
            }),
            None,
            0,
            1,
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(150),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(150),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(150),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(150),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "150"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "2"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "ask_fee_account".into(),
                        amount: coins(2, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(148, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(150, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_round_down() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_custom_test_base_contract_v3(
            &mut deps.storage,
            None,
            Some(FeeInfo {
                account: Addr::unchecked("bid_fee_account"),
                rate: "0.01".into(),
            }),
            0,
            1,
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(1),
                    denom: "quote_1".into(),
                }),
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "1"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: coins(1, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(149, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_not_applicable() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_custom_test_base_contract_v3(
            &mut deps.storage,
            None,
            Some(FeeInfo {
                account: Addr::unchecked("bid_fee_account"),
                rate: "0.01".into(),
            }),
            0,
            1,
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order without fees
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(149, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_round_up() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(150),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(150),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(2),
                    denom: "quote_1".into(),
                }),
                quote: Coin {
                    amount: Uint128::new(150),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(150),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "150"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "2"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: coins(2, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(150, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(150, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_ask_order() {
        // setup
        let mut deps = mock_provenance_dependencies_with_custom_querier(
            MockProvenanceQuerier::new(&[(MOCK_CONTRACT_ADDR, &[coin(30, "base_1")])]));
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(30),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(20),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(10),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "10"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(20, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(10, "base_1"),
                    })
                );
            }
        }

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "base_1".into(),
                        class: AskOrderClass::Basic,
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(20)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_bid_order() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(50),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
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
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(50),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "50"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(100, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(50, "base_1"),
                    })
                );
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(50),
                        accumulated_quote: Uint128::new(100),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(200),
                            denom: "quote_1".into(),
                        },
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_both_orders() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(300),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(600),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
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

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "base_1".into(),
                        class: AskOrderClass::Basic,
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        base: Coin {
                            amount: Uint128::new(300),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(100),
                        accumulated_quote: Uint128::new(200),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(600),
                            denom: "quote_1".into(),
                        },
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn execute_convertible_partial_both_orders() {
        // setup
        let mut deps = mock_provenance_dependencies();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
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
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_2"),
                        converted_base: coin(200, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(300),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(600),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(100, "con_base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(200, "quote_1")],
                    })
                );
            }
        }

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_2"),
                                converted_base: coin(100, "base_1"),
                            }
                        },

                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        base: Coin {
                            amount: Uint128::new(300),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(100),
                        accumulated_quote: Uint128::new(200),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(600),
                            denom: "quote_1".into(),
                        },
                    }
                );
                assert_eq!(400_u128, stored_order.get_remaining_quote().u128());
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // difference
    #[test]
    fn execute_price_overlap_use_ask() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(5),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(500),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(
                    execute_response.attributes[5],
                    attr("price", "2.000000000000000000")
                );
                assert_eq!(execute_response.attributes[6], attr("size", "5"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // difference
    #[test]
    fn execute_price_overlap_use_ask_with_partial_bid() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(1000),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(
                    execute_response.attributes[5],
                    attr("price", "2.000000000000000000")
                );
                assert_eq!(execute_response.attributes[6], attr("size", "5"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order update
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        base: Coin {
                            amount: Uint128::new(10),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(5),
                        accumulated_quote: Uint128::new(10 + 490),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "100.000000000000000000".into(),
                        quote: Coin {
                            amount: Uint128::new(1000),
                            denom: "quote_1".into(),
                        },
                    }
                );
                assert_eq!(stored_order.get_remaining_base().u128(), 5_u128);
                assert_eq!(stored_order.get_remaining_quote().u128(), 500_u128);
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // partial quote and partial fee
    #[test]
    fn execute_price_overlap_use_ask_with_bid_fees() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".to_string(),
                }),
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(1000),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(
                    execute_response.attributes[5],
                    attr("price", "2.000000000000000000")
                );
                assert_eq!(execute_response.attributes[6], attr("size", "5"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "1"));

                assert_eq!(execute_response.messages.len(), 5);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: vec![coin(1, "quote_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[3].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[4].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(49, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(10),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(5),
                        accumulated_quote: Uint128::new(10 + 490),
                        accumulated_fee: Uint128::new(1 + 49),
                        fee: Some(Coin {
                            amount: Uint128::new(100),
                            denom: "quote_1".to_string(),
                        }),
                        quote: Coin {
                            denom: "quote_1".into(),
                            amount: Uint128::new(1000),
                        },
                        price: "100.000000000000000000".into(),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // remaining quote balance if remaining order size = 0
    #[test]
    fn execute_price_overlap_use_ask_and_quote_restricted() {
        // setup
        let mut deps = mock_provenance_dependencies();
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

        let quote_marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _quote_marker: MarkerAccount = from_binary(&Binary::from(quote_marker_json)).unwrap();
        // deps.querier.with_markers(vec![quote_marker]); // TODO: find alternative function

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(5),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(500),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(
                    execute_response.attributes[5],
                    attr("price", "2.000000000000000000")
                );
                assert_eq!(execute_response.attributes[6], attr("size", "5"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        10,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        490,
                        "quote_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_price_overlap_use_bid() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
                price: "4".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(400, "quote_1"),
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
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
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
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_denom"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_denom"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(100, "con_base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_ask() {
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _test_marker: MarkerAccount = from_binary(&Binary::from(marker_json)).unwrap();
        // deps.querier.with_markers(vec![test_marker]); // TODO: find alternative function

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_bid() {
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let quote_marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _quote_marker: MarkerAccount = from_binary(&Binary::from(quote_marker_json)).unwrap();
        // deps.querier.with_markers(vec![quote_marker]); // TODO: find alternative function

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(100, "base_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_ask_and_bid() {
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let base_marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let quote_marker_json = b"{
              \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 11,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _base_marker: MarkerAccount = from_binary(&Binary::from(base_marker_json)).unwrap();
        let _quote_marker: MarkerAccount = from_binary(&Binary::from(quote_marker_json)).unwrap();
        // deps.querier.with_markers(vec![base_marker, quote_marker]); // TODO: find alternative function

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_base_restricted_marker() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        let restricted_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let restricted_con_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"con_base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"con_base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _marker_base_1: MarkerAccount = from_binary(&Binary::from(restricted_base_1)).unwrap();
        let _marker_con_base_1: MarkerAccount = from_binary(&Binary::from(restricted_con_base_1)).unwrap();
        // deps.querier.with_markers(vec![marker_base_1, marker_con_base_1]); // TODO: find alternative function

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_quote_restricted_marker() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        let quote_marker_json = b"{
              \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 11,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _marker_quote_1: MarkerAccount = from_binary(&Binary::from(quote_marker_json)).unwrap();
        // deps.querier.with_markers(vec![marker_quote_1]); // TODO: find alternative function

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(100, "base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(100, "con_base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_base_and_quote_restricted_marker() {
        // setup
        let mut deps = mock_provenance_dependencies();
        let mock_env = mock_env();

        let restricted_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let restricted_con_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"con_base_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 10,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"con_base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let restricted_quote_marker_json = b"{
              \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 11,
              \"sequence\": 0,
              \"permissions\": [
                {
                  \"permissions\": [
                    \"burn\",
                    \"delete\",
                    \"deposit\",
                    \"admin\",
                    \"mint\",
                    \"withdraw\"
                  ],
                  \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let _marker_base_1: MarkerAccount = from_binary(&Binary::from(restricted_base_1)).unwrap();
        let _marker_con_base_1: MarkerAccount = from_binary(&Binary::from(restricted_con_base_1)).unwrap();
        let _marker_quote_1: MarkerAccount =
            from_binary(&Binary::from(restricted_quote_marker_json)).unwrap();
        // TODO: find alternative function
        // deps.querier
        //     .with_markers(vec![marker_base_1, marker_con_base_1, marker_quote_1]);

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
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
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_invalid_data() {
        // setup
        let mut deps = mock_provenance_dependencies();
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

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "".into(),
            bid_id: "".into(),
            price: "0".into(),
            size: Uint128::zero(),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"ask_id".into()));
                    assert!(fields.contains(&"bid_id".into()));
                }
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_by_non_executor() {
        // setup
        let mut deps = mock_provenance_dependencies();
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

        // execute by non-executor
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(1),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::Unauthorized) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_ask_not_ready() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute when ask order not ready returns ContractError::PendingIssuerApproval
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::AskOrderNotReady { current_status }) => {
                assert_eq!(
                    current_status,
                    format!("{:?}", AskOrderStatus::PendingIssuerApproval)
                )
            }
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_ask_non_exist() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on non-existent ask order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::LoadOrderFailed { .. }) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_bid_non_exist() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // execute on non-existent bid order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::LoadOrderFailed { .. }) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_with_sent_funds() {
        // setup
        let mut deps = mock_provenance_dependencies();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                supported_quote_denoms: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // execute with sent_funds returns ContractError::ExecuteWithFunds
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(1),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &coins(100, "funds")),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::ExecuteWithFunds) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_price_mismatch() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "3".into(),
                quote: "quote_1".into(),
                size: Uint128::new(300),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(ContractError::AskBidPriceMismatch) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_price_not_ask_or_bid() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "6".into(),
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
            Err(ContractError::InvalidExecutePrice) => (),
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order still exists
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }

    #[test]
    fn execute_size_greater_than_ask_and_bid() {
        // setup
        let mut deps = mock_provenance_dependencies();
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(ContractError::InvalidExecuteSize) => (),
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order still exists
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }
}
