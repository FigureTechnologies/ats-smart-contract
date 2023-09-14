#[cfg(test)]
mod cancel_ask_tests {
    use crate::ask_order::{AskOrderClass, AskOrderStatus, AskOrderV1, ASKS_V1};
    use crate::bid_order::BidOrderV3;
    use crate::contract::execute;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_setup_utils::{
        setup_test_base_contract_v3, store_test_ask, store_test_bid,
    };
    use crate::tests::test_utils::setup_asset_marker;
    use crate::util::transfer_marker_coins;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, coins, Addr, BankMsg, Coin, CosmosMsg, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::types::provenance::marker::v1::QueryMarkerRequest;
    use std::convert::TryInto;

    #[test]
    fn cancel_ask_valid() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

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

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: HYPHENATED_ASK_ID.to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", HYPHENATED_ASK_ID)
                );
                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1
            .load(&deps.storage, HYPHENATED_ASK_ID.as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_ask_empty_id_string_returns_err() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk { id: "".to_string() };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["id".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn cancel_ask_valid_legacy_unhyphenated_id_then_cancels_order() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: UNHYPHENATED_ASK_ID.into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: UNHYPHENATED_ASK_ID.to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", UNHYPHENATED_ASK_ID)
                );
                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1
            .load(&deps.storage, UNHYPHENATED_ASK_ID.as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_ask_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        QueryMarkerRequest::mock_response(
            &mut deps.querier,
            setup_asset_marker(
                "tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u".to_string(),
                "tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz".to_string(),
                "base_1".to_string(),
            ),
        );

        // create bid data
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("asker"),
                class: AskOrderClass::Basic,
                base: "base_1".into(),
                quote: "quote_1".into(),
                price: "2".into(),
                size: Uint128::new(100),
            },
        );

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_ask_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );

                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                    )
                    .unwrap()
                    .try_into()
                    .unwrap()
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1
            .load(
                &deps.storage,
                "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()
            )
            .is_err());
    }

    #[test]
    fn cancel_ask_convertible_valid() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

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

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(cancel_ask_response.messages.len(), 2);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(100, "con_base_1"),
                    })
                );
                assert_eq!(
                    cancel_ask_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".to_string(),
                        amount: coins(100, "base_denom"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1
            .load(
                &deps.storage,
                "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()
            )
            .is_err());
    }

    #[test]
    fn cancel_ask_convertible_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // TODO - fix test since mock response returns same result no matter the input
        QueryMarkerRequest::mock_response(
            &mut deps.querier,
            setup_asset_marker(
                "tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u".to_string(),
                "tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz".to_string(),
                "con_base_1".to_string(),
            ),
        );
        QueryMarkerRequest::mock_response(
            &mut deps.querier,
            setup_asset_marker(
                "tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u".to_string(),
                "tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz".to_string(),
                "base_1".to_string(),
            ),
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

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );

                assert_eq!(cancel_ask_response.messages.len(), 2);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        asker_info.sender,
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                    )
                    .unwrap()
                    .try_into()
                    .unwrap()
                );
                assert_eq!(
                    cancel_ask_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                    )
                    .unwrap()
                    .try_into()
                    .unwrap()
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1
            .load(
                &deps.storage,
                "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()
            )
            .is_err());
    }

    #[test]
    fn cancel_ask_invalid_data() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let asker_info = mock_info("asker", &[]);

        // cancel ask order with missing id returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk { id: "".to_string() };
        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()))
                }
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_non_exist() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let asker_info = mock_info("asker", &[]);

        // cancel non-existent ask order returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::LoadOrderFailed { .. } => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_sender_notequal() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let asker_info = mock_info("asker", &[]);

        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("not_asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // cancel ask order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_with_sent_funds() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // cancel ask order with sent_funds returns ContractError::CancelWithFunds
        let asker_info = mock_info("asker", &coins(1, "sent_coin"));
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::CancelWithFunds => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_partial() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let bid_id = "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b";
        let ask_id = "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367";

        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: ask_id.into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                id: bid_id.into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(20),
                    denom: "quote_1".into(),
                },
            },
        );

        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: ask_id.into(),
            bid_id: bid_id.into(),
            price: "2".into(),
            size: Uint128::new(10),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(..) => {}
        }

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: ask_id.to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(cancel_ask_response.attributes[1], attr("id", ask_id));
                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(90, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        assert!(ASKS_V1.load(&deps.storage, ask_id.as_bytes()).is_err());
    }
}
