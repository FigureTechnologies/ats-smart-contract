#[cfg(test)]
mod cancel_bid_tests {
    use crate::ask_order::{AskOrderClass, AskOrderV1};
    use crate::bid_order::{get_bid_storage_read, BidOrderV3};
    use crate::common::FeeInfo;
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_BID_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_setup_utils::{
        setup_test_base, setup_test_base_contract_v3, store_test_ask, store_test_bid,
    };
    use crate::util::{transfer_marker_coins};
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coins, from_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::types::provenance::marker::v1::MarkerAccount;

    #[test]
    fn cancel_bid_valid() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
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
                id: HYPHENATED_BID_ID.into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: HYPHENATED_BID_ID.to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", HYPHENATED_BID_ID)
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage.load(HYPHENATED_BID_ID.as_bytes()).is_err());
    }

    #[test]
    fn cancel_bid_legacy_unhyphenated_id_then_cancels_order() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
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
                id: UNHYPHENATED_BID_ID.into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: UNHYPHENATED_BID_ID.to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", UNHYPHENATED_BID_ID)
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage.load(UNHYPHENATED_BID_ID.as_bytes()).is_err());
    }

    #[test]
    fn cancel_bid_restricted_marker() {
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

        let marker_json = b"{
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

        let _test_marker: MarkerAccount = from_binary(&Binary::from(marker_json)).unwrap();
        // deps.querier.with_markers(vec![test_marker]); // TODO: find alternative function

        // create bid data
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
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );

                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    transfer_marker_coins(
                        200,
                        "quote_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_bid_with_fees_valid() {
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
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        // create bid data
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
                fee: Some(Coin {
                    denom: "quote_1".to_string(),
                    amount: Uint128::new(20),
                }),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(cancel_bid_response.messages.len(), 2);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    cancel_bid_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(20, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_bid_with_fees_eq_zero_valid() {
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
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        // create bid data
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
                fee: Some(Coin {
                    denom: "quote_1".to_string(),
                    amount: Uint128::new(0),
                }),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_bid_restricted_marker_with_fees() {
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
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        let marker_json = b"{
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

        let _test_marker: MarkerAccount = from_binary(&Binary::from(marker_json)).unwrap();
        // deps.querier.with_markers(vec![test_marker]); // TODO: find alternative function

        // create bid data
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
                fee: Some(Coin {
                    amount: Uint128::new(20),
                    denom: "quote_1".to_string(),
                }),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );

                assert_eq!(cancel_bid_response.messages.len(), 2);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    transfer_marker_coins(
                        200,
                        "quote_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    cancel_bid_response.messages[1].msg,
                    transfer_marker_coins(
                        20,
                        "quote_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn cancel_bid_invalid_data() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let bidder_info = mock_info("bidder", &[]);

        // cancel bid order with missing id returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelAsk { id: "".to_string() };
        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

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
    fn cancel_bid_non_exist() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let bidder_info = mock_info("bidder", &[]);

        // cancel non-existent bid order returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

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
    fn cancel_bid_sender_notequal() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let bidder_info = mock_info("bidder", &[]);

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
                owner: Addr::unchecked("not_bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
            },
        );

        // cancel bid order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

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
    fn cancel_bid_with_sent_funds() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // cancel bid order with sent_funds returns ContractError::CancelWithFunds
        let bidder_info = mock_info("bidder", &coins(1, "sent_coin"));
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

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
    fn cancel_bid_partial() {
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
                size_increment: Uint128::new(10),
            },
        );

        let bid_id = "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b";
        let ask_id = "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367";

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
                id: bid_id.into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
            },
        );

        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: ask_id.into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(10),
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

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: bid_id.to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 4);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(cancel_bid_response.attributes[1], attr("id", bid_id));
                assert_eq!(
                    cancel_bid_response.attributes[2],
                    attr("reverse_size", "90")
                );
                assert_eq!(
                    cancel_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(180, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        assert!(bid_storage.load(bid_id.as_bytes()).is_err());
    }
}
