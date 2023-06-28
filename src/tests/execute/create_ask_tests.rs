#[cfg(test)]
mod create_ask_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderStatus, AskOrderV1};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::UNHYPHENATED_ASK_ID;
    use crate::tests::test_setup_utils::{setup_test_base, setup_test_base_contract_v3};
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use crate::util::{transfer_marker_coins};
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, coins, from_binary, Addr, Binary, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::types::provenance::marker::v1::MarkerAccount;

    #[test]
    fn create_ask_invalid_input_unhyphenated_id() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: UNHYPHENATED_ASK_ID.into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128::new(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_1"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info,
            create_ask_msg.clone(),
        );

        // verify create ask response
        validate_execute_invalid_id_field(create_ask_response)
    }

    #[test]
    fn create_ask_valid_data() {
        let mut deps = mock_provenance_dependencies();
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

        // TODO: find alternative function
        // deps.querier.with_attributes(
        //     "asker",
        //     &[
        //         ("ask_tag_1", "ask_tag_1_value", "String"),
        //         ("ask_tag_2", "ask_tag_2_value", "String"),
        //     ],
        // );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128::new(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_1"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info,
            create_ask_msg.clone(),
        );

        // verify create ask response
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Basic {}).unwrap()
                    )
                );
                assert_eq!(response.attributes[3], attr("target_base", "base_1"));
                assert_eq!(response.attributes[4], attr("base", "base_1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("price", "2.5"));
                assert_eq!(response.attributes[7], attr("size", "200"));
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify ask order stored
        let ask_storage = get_ask_storage_read(&deps.storage);
        if let ExecuteMsg::CreateAsk {
            id,
            base,
            quote,
            price,
            size,
        } = create_ask_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id,
                            owner: Addr::unchecked("asker"),
                            price,
                            quote,
                            size
                        }
                    )
                }
                _ => {
                    panic!("ask order was not found in storage")
                }
            }
        } else {
            panic!("ask_message is not a CreateAsk type. this is bad.")
        }
    }

    #[test]
    fn create_ask_convertible_base() {
        let mut deps = mock_provenance_dependencies();
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

        // TODO: find alternative function
        // deps.querier.with_attributes(
        //     "asker",
        //     &[
        //         ("ask_tag_1", "ask_tag_1_value", "String"),
        //         ("ask_tag_2", "ask_tag_2_value", "String"),
        //     ],
        // );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "con_base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(500),
        };

        let asker_info = mock_info("asker", &coins(500, "con_base_1"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );

        // verify create ask response
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval
                        })
                        .unwrap()
                    )
                );
                assert_eq!(response.attributes[3], attr("target_base", "base_1"));
                assert_eq!(response.attributes[4], attr("base", "con_base_1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("price", "2"));
                assert_eq!(response.attributes[7], attr("size", "500"));
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify ask order stored
        let ask_storage = get_ask_storage_read(&deps.storage);
        if let ExecuteMsg::CreateAsk {
            id,
            base,
            quote,
            price,
            size,
        } = create_ask_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Convertible {
                                status: AskOrderStatus::PendingIssuerApproval,
                            },
                            id,
                            owner: asker_info.sender,
                            price,
                            quote,
                            size,
                        }
                    )
                }
                _ => {
                    panic!("ask order was not found in storage")
                }
            }
        } else {
            panic!("ask_message is not a CreateAsk type. this is bad.")
        }
    }

    #[test]
    fn create_ask_with_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
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
              \"supply_fixed\": false
            }";

        let _test_marker: MarkerAccount = from_binary(&Binary::from(marker_json)).unwrap();
        // deps.querier.with_markers(vec![test_marker]); // TODO: find alternative function

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(500),
        };

        let asker_info = mock_info("asker", &[]);

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );

        // verify create ask response
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Basic {}).unwrap()
                    )
                );
                assert_eq!(response.attributes[3], attr("target_base", "base_1"));
                assert_eq!(response.attributes[4], attr("base", "base_1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("price", "2"));
                assert_eq!(response.attributes[7], attr("size", "500"));

                assert_eq!(response.messages.len(), 1);
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        500,
                        "base_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("asker")
                    )
                    .unwrap()
                );
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify ask order stored
        let ask_storage = get_ask_storage_read(&deps.storage);
        if let ExecuteMsg::CreateAsk {
            id,
            base,
            quote,
            price,
            size,
        } = create_ask_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id,
                            owner: asker_info.sender,
                            price,
                            quote,
                            size,
                        }
                    )
                }
                _ => {
                    panic!("ask order was not found in storage")
                }
            }
        } else {
            panic!("ask_message is not a CreateAsk type. this is bad.")
        }
    }

    #[test]
    fn create_ask_with_restricted_marker_with_funds() {
        let mut deps = mock_provenance_dependencies();
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
              \"supply_fixed\": false
            }";

        let _test_marker: MarkerAccount = from_binary(&Binary::from(marker_json)).unwrap();
        // deps.querier.with_markers(vec![test_marker]); // TODO: find alternative function

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(500),
        };

        let asker_info = mock_info("asker", &[coin(10, "base_1")]);

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                create_ask_response
            ),
        }
    }

    #[test]
    fn create_ask_existing_id() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2.5".into(),
            size: Uint128::new(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_1"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Ok(_) => {}
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // create ask data with existing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_2".into(),
            price: "4.5".into(),
            size: Uint128::new(400),
        };

        let asker_info = mock_info("asker", &coins(400, "base_1"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify ask order stored is the original order
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
                        price: "2.5".into(),
                        quote: "quote_1".into(),
                        size: Uint128::new(200)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn create_ask_invalid_data() {
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

        // create ask missing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "".into(),
            base: "".to_string(),
            quote: "".into(),
            price: "".into(),
            size: Uint128::new(0),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(0, "base_1")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                    assert!(fields.contains(&"base".into()));
                    assert!(fields.contains(&"quote".into()));
                    assert!(fields.contains(&"price".into()));
                    assert!(fields.contains(&"size".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_inconvertible_base() {
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

        // create ask with inconvertible base
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "inconvertible".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(100, "inconvertible")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InconvertibleBaseDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_unsupported_quote() {
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

        // create ask with unsupported quote
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "unsupported".into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(100, "base_denom")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedQuoteDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_invalid_price_negative() {
        let mut deps = mock_provenance_dependencies();
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

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "-2.5".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128::new(200),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(200, "base_1")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"price".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_invalid_price_zero() {
        let mut deps = mock_provenance_dependencies();
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

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "0".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128::new(200),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(200, "base_1")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"price".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_invalid_price_precision() {
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create ask
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2.123".into(),
            size: Uint128::new(500),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(500, "base_denom")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"price".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_wrong_account_attributes() {
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

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_denom"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::Unauthorized => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }
}
