#[cfg(test)]
mod reject_ask_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderV1};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_setup_utils::{
        setup_test_base, setup_test_base_contract_v3, store_test_ask,
    };
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, coins, Addr, BankMsg, CosmosMsg, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn reject_ask_valid() {
        let mut deps = mock_dependencies(&[]);
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: HYPHENATED_ASK_ID.to_string(),
            size: None,
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        match reject_ask_response {
            Ok(reject_ask_response) => {
                assert_eq!(reject_ask_response.attributes.len(), 4);
                assert_eq!(
                    reject_ask_response.attributes[0],
                    attr("action", "reject_ask")
                );
                assert_eq!(
                    reject_ask_response.attributes[1],
                    attr("id", HYPHENATED_ASK_ID)
                );
                assert_eq!(
                    reject_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(reject_ask_response.messages.len(), 1);
                assert_eq!(
                    reject_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage.load(HYPHENATED_ASK_ID.as_bytes()).is_err());
    }

    #[test]
    fn reject_ask_legacy_unhyphenated_id_then_rejects_ask() {
        let mut deps = mock_dependencies(&[]);
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: UNHYPHENATED_ASK_ID.to_string(),
            size: None,
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        match reject_ask_response {
            Ok(reject_ask_response) => {
                assert_eq!(reject_ask_response.attributes.len(), 4);
                assert_eq!(
                    reject_ask_response.attributes[0],
                    attr("action", "reject_ask")
                );
                assert_eq!(
                    reject_ask_response.attributes[1],
                    attr("id", UNHYPHENATED_ASK_ID)
                );
                assert_eq!(
                    reject_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(reject_ask_response.messages.len(), 1);
                assert_eq!(
                    reject_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage.load(UNHYPHENATED_ASK_ID.as_bytes()).is_err());
    }

    #[test]
    fn reject_partial_ask_valid() {
        let mut deps = mock_dependencies(&[]);
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
                size: Uint128::new(200),
            },
        );

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
            size: Some(Uint128::new(100)),
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        match reject_ask_response {
            Ok(reject_ask_response) => {
                assert_eq!(reject_ask_response.attributes.len(), 4);
                assert_eq!(
                    reject_ask_response.attributes[0],
                    attr("action", "reject_ask")
                );
                assert_eq!(
                    reject_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    reject_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_ask_response.attributes[3],
                    attr("order_open", "true")
                );
                assert_eq!(reject_ask_response.messages.len(), 1);
                assert_eq!(
                    reject_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
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
    }

    #[test]
    fn reject_partial_ask_cancel_size_not_increment() {
        let mut deps = mock_dependencies(&[]);
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
            size: Some(Uint128::new(50)),
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        match reject_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"size".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify ask order unchanged
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
    }

    #[test]
    fn reject_partial_ask_cancel_size_greater_than_order_size() {
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
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(100),
            },
        );

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let reject_ask_msg = ExecuteMsg::RejectAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
            size: Some(Uint128::new(150)),
        };
        let reject_ask_response = execute(deps.as_mut(), mock_env(), exec_info, reject_ask_msg);

        match reject_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"size".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify ask order unchanged
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
    }
}
