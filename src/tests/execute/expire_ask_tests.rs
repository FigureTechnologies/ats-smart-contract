#[cfg(test)]
mod expire_ask_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderStatus, AskOrderV1};
    use crate::contract::execute;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_setup_utils::{setup_test_base_contract_v3, store_test_ask};
    use crate::util::transfer_marker_coins;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, coins, from_binary, Addr, BankMsg, Binary, CosmosMsg, Uint128};
    use prost::Message;
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::shim::Any;
    use provwasm_std::types::cosmos::auth::v1beta1::BaseAccount;
    use provwasm_std::types::provenance::marker::v1::{
        AccessGrant, MarkerAccount, MarkerStatus, MarkerType, QueryMarkerRequest,
        QueryMarkerResponse,
    };
    use std::convert::TryInto;

    #[test]
    fn expire_ask_valid() {
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: HYPHENATED_ASK_ID.to_string(),
        };
        let expire_ask_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_ask_response {
            Ok(expire_ask_response) => {
                assert_eq!(expire_ask_response.attributes.len(), 4);
                assert_eq!(
                    expire_ask_response.attributes[0],
                    attr("action", "expire_ask")
                );
                assert_eq!(
                    expire_ask_response.attributes[1],
                    attr("id", HYPHENATED_ASK_ID)
                );
                assert_eq!(
                    expire_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_ask_response.messages.len(), 1);
                assert_eq!(
                    expire_ask_response.messages[0].msg,
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
    fn expire_ask_legacy_unhyphenated_id_then_expires_ask() {
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: UNHYPHENATED_ASK_ID.to_string(),
        };
        let expire_ask_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_ask_response {
            Ok(expire_ask_response) => {
                assert_eq!(expire_ask_response.attributes.len(), 4);
                assert_eq!(
                    expire_ask_response.attributes[0],
                    attr("action", "expire_ask")
                );
                assert_eq!(
                    expire_ask_response.attributes[1],
                    attr("id", UNHYPHENATED_ASK_ID)
                );
                assert_eq!(
                    expire_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_ask_response.messages.len(), 1);
                assert_eq!(
                    expire_ask_response.messages[0].msg,
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
    fn expire_ask_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        QueryMarkerRequest::mock_response(&mut deps.querier, setup_asset_marker());

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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_ask_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_ask_response {
            Ok(expire_ask_response) => {
                assert_eq!(expire_ask_response.attributes.len(), 4);
                assert_eq!(
                    expire_ask_response.attributes[0],
                    attr("action", "expire_ask")
                );
                assert_eq!(
                    expire_ask_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    expire_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_ask_response.messages.len(), 1);
                assert_eq!(
                    expire_ask_response.messages[0].msg,
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
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn expire_ask_convertible_valid() {
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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let expire_ask_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_ask_response {
            Ok(expire_ask_response) => {
                assert_eq!(expire_ask_response.attributes.len(), 4);
                assert_eq!(
                    expire_ask_response.attributes[0],
                    attr("action", "expire_ask")
                );
                assert_eq!(
                    expire_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    expire_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_ask_response.messages.len(), 2);
                assert_eq!(
                    expire_ask_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".to_string(),
                        amount: coins(100, "con_base_1"),
                    })
                );
                assert_eq!(
                    expire_ask_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".to_string(),
                        amount: coins(100, "base_denom"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());
    }

    #[test]
    fn expire_ask_convertible_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);
        // todo should mock 2 markers
        QueryMarkerRequest::mock_response(&mut deps.querier, setup_asset_marker());

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

        // expire ask order
        let exec_info = mock_info("exec_1", &[]);

        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let expire_ask_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_ask_response {
            Ok(expire_ask_response) => {
                assert_eq!(expire_ask_response.attributes.len(), 4);
                assert_eq!(
                    expire_ask_response.attributes[0],
                    attr("action", "expire_ask")
                );
                assert_eq!(
                    expire_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    expire_ask_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_ask_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_ask_response.messages.len(), 2);
                assert_eq!(
                    expire_ask_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                    )
                    .unwrap()
                    .try_into()
                    .unwrap()
                );
                assert_eq!(
                    expire_ask_response.messages[1].msg,
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
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());
    }

    #[test]
    fn expire_ask_invalid_data() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("exec_1", &[]);

        // expire ask order with missing id returns ContractError::Unauthorized
        let expire_ask_msg = ExecuteMsg::ExpireAsk { id: "".to_string() };
        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_response {
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
    fn expire_ask_non_exist() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("exec_1", &[]);

        // expire non-existent ask order returns ContractError::Unauthorized
        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_response {
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
    fn expire_ask_sender_notequal_to_owner() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("not_exec", &[]);

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

        // expire ask order with sender not equal to owner returns ContractError::Unauthorized
        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_response {
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
    fn expire_ask_with_sent_funds() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // expire ask order with sent_funds returns ContractError::ExpireWithFunds
        let exec_info = mock_info("exec_1", &coins(1, "sent_coin"));
        let expire_ask_msg = ExecuteMsg::ExpireAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_ask_msg);

        match expire_response {
            Err(error) => match error {
                ContractError::ExpireWithFunds => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    fn setup_asset_marker() -> QueryMarkerResponse {
        let expected_marker: MarkerAccount = MarkerAccount {
            base_account: Some(BaseAccount {
                address: "tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u".to_string(),
                pub_key: None,
                account_number: 10,
                sequence: 0,
            }),
            manager: "".to_string(),
            access_control: vec![AccessGrant {
                address: "tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz".to_string(),
                permissions: vec![1, 2, 3, 4, 5, 6, 7],
            }],
            status: MarkerStatus::Active.into(),
            denom: "base_1".to_string(),
            supply: "1000".to_string(),
            marker_type: MarkerType::Restricted.into(),
            supply_fixed: false,
            allow_governance_control: true,
            allow_forced_transfer: false,
            required_attributes: vec![],
        };

        QueryMarkerResponse {
            marker: Some(Any {
                type_url: "/provenance.marker.v1.MarkerAccount".to_string(),
                value: expected_marker.encode_to_vec(),
            }),
        }
    }
}
