#[cfg(test)]
mod approve_ask_tests {
    use crate::ask_order::{AskOrderClass, AskOrderStatus, AskOrderV1, ASKS_V1};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{
        APPROVER_1, BASE_DENOM, HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID,
    };
    use crate::tests::test_setup_utils::{
        setup_test_base, setup_test_base_contract_v3, store_test_ask,
    };
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, from_binary, Addr, Binary, Uint128};
    use provwasm_mocks::mock_dependencies;
    use provwasm_std::{transfer_marker_coins, Marker};

    #[test]
    fn approve_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: UNHYPHENATED_ASK_ID.into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // verify execute approve ask response
        validate_execute_invalid_id_field(approve_ask_response)
    }

    #[test]
    fn approve_ask_valid() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(approve_ask_response) => {
                assert_eq!(approve_ask_response.attributes.len(), 6);
                assert_eq!(
                    approve_ask_response.attributes[0],
                    attr("action", "approve_ask")
                );
                assert_eq!(
                    approve_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    approve_ask_response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_denom")
                            },
                        })
                        .unwrap()
                    )
                );
                assert_eq!(approve_ask_response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(approve_ask_response.attributes[4], attr("price", "2"));
                assert_eq!(approve_ask_response.attributes[5], attr("size", "100"));
                assert_eq!(approve_ask_response.messages.len(), 0);
            }
        }

        // verify ask order update
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
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
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_already_approved_return_err() {
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

        // store approved ask order
        let existing_ask_order = AskOrderV1 {
            base: "con_base_1".into(),
            class: AskOrderClass::Convertible {
                status: AskOrderStatus::Ready {
                    // Already marked ready
                    approver: Addr::unchecked("approver_1"),
                    converted_base: coin(100, "base_denom"),
                },
            },
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        store_test_ask(&mut deps.storage, &existing_ask_order);

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_2", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::AskOrderReady { approver } => {
                    assert_eq!("approver_1", approver)
                }
                _ => panic!("unexpected error type: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order the same
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(stored_order, existing_ask_order)
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_restricted_marker() {
        // setup
        let mut deps = mock_dependencies(&[]);
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
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
              \"allow_forced_transfer\": false,
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(approve_ask_response) => {
                assert_eq!(approve_ask_response.attributes.len(), 6);
                assert_eq!(
                    approve_ask_response.attributes[0],
                    attr("action", "approve_ask")
                );
                assert_eq!(
                    approve_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    approve_ask_response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_1")
                            },
                        })
                        .unwrap()
                    )
                );
                assert_eq!(approve_ask_response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(approve_ask_response.attributes[4], attr("price", "2"));
                assert_eq!(approve_ask_response.attributes[5], attr("size", "100"));

                assert_eq!(approve_ask_response.messages.len(), 1);
                assert_eq!(
                    approve_ask_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("approver_1")
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order update
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
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
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_id() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "59e82f8f-268e-433f-9711-e9f2d2cc19a5".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_converted_base_denom() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "wrong_base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "wrong_base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_converted_base_amount() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(101, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_converted_base_amount_sent_funds_mismatch() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(99),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_restricted_marker_with_funds() {
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
              \"allow_forced_transfer\": false,
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(10, "gme")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                approve_ask_response
            ),
        }
    }

    #[test]
    fn approve_ask_restricted_marker_order_size_mismatch() {
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
              \"allow_forced_transfer\": false,
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(101),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                approve_ask_response
            ),
        }

        // verify ask order update
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_not_approver() {
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
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("not_approver", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        match ASKS_V1.load(
            &deps.storage,
            "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_with_basic_class_returns_err() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: BASE_DENOM.into(),
                class: AskOrderClass::Basic, // Only AskOrderClass::Convertible should accept an approve
                id: HYPHENATED_ASK_ID.into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info(APPROVER_1, &[coin(100, BASE_DENOM)]),
            // mock_info(APPROVER_1, &[]),
            ExecuteMsg::ApproveAsk {
                id: HYPHENATED_ASK_ID.into(),
                base: BASE_DENOM.to_string(),
                size: Uint128::new(100),
            },
        );

        // verify approve failed ask response
        match approve_ask_response {
            Ok(_) => {
                panic!("Expected error but got Ok")
            }
            Err(error) => match error {
                ContractError::InconvertibleBaseDenom {} => {}
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }
}
