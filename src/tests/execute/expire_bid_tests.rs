#[cfg(test)]
mod expire_bid_tests {
    use crate::bid_order::{BidOrderV3, BIDS_V3};
    use crate::common::FeeInfo;
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_BID_ID, UNHYPHENATED_BID_ID};
    use crate::tests::test_setup_utils::{
        setup_test_base, setup_test_base_contract_v3, store_test_bid,
    };
    use crate::tests::test_utils::setup_restricted_asset_marker;
    use crate::util::transfer_marker_coins;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coins, Addr, BankMsg, Coin, CosmosMsg, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::types::provenance::marker::v1::QueryMarkerRequest;
    use std::convert::TryInto;

    #[test]
    fn expire_bid_valid() {
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

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: HYPHENATED_BID_ID.to_string(),
        };

        let expire_bid_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

        match expire_bid_response {
            Ok(expire_bid_response) => {
                assert_eq!(expire_bid_response.attributes.len(), 4);
                assert_eq!(
                    expire_bid_response.attributes[0],
                    attr("action", "expire_bid")
                );
                assert_eq!(
                    expire_bid_response.attributes[1],
                    attr("id", HYPHENATED_BID_ID)
                );
                assert_eq!(
                    expire_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_bid_response.messages.len(), 1);
                assert_eq!(
                    expire_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        assert!(BIDS_V3
            .load(&deps.storage, HYPHENATED_BID_ID.as_bytes())
            .is_err());
    }

    #[test]
    fn expire_bid_legacy_unhyphenated_id_then_expires_bid() {
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

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: UNHYPHENATED_BID_ID.to_string(),
        };

        let expire_bid_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

        match expire_bid_response {
            Ok(expire_bid_response) => {
                assert_eq!(expire_bid_response.attributes.len(), 4);
                assert_eq!(
                    expire_bid_response.attributes[0],
                    attr("action", "expire_bid")
                );
                assert_eq!(
                    expire_bid_response.attributes[1],
                    attr("id", UNHYPHENATED_BID_ID)
                );
                assert_eq!(
                    expire_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_bid_response.messages.len(), 1);
                assert_eq!(
                    expire_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        assert!(BIDS_V3
            .load(&deps.storage, UNHYPHENATED_BID_ID.as_bytes())
            .is_err());
    }

    #[test]
    fn expire_bid_restricted_marker() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        QueryMarkerRequest::mock_response(
            &mut deps.querier,
            setup_restricted_asset_marker(
                "tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u".to_string(),
                "tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz".to_string(),
                "quote_1".to_string(),
            ),
        );

        // create bid data
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

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_bid_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

        match expire_bid_response {
            Ok(expire_bid_response) => {
                assert_eq!(expire_bid_response.attributes.len(), 4);
                assert_eq!(
                    expire_bid_response.attributes[0],
                    attr("action", "expire_bid")
                );
                assert_eq!(
                    expire_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    expire_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    expire_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(expire_bid_response.messages.len(), 1);
                assert_eq!(
                    expire_bid_response.messages[0].msg,
                    transfer_marker_coins(
                        200,
                        "quote_1",
                        Addr::unchecked("bidder"),
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

        // verify bid order removed from storage
        assert!(BIDS_V3
            .load(
                &deps.storage,
                "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()
            )
            .is_err());
    }

    #[test]
    fn expire_bid_invalid_data() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("exec_1", &[]);

        // expire bid order with missing id returns ContractError::Unauthorized
        let expire_bid_msg = ExecuteMsg::ExpireAsk { id: "".to_string() };
        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

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
    fn expire_bid_non_exist() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("exec_1", &[]);

        // expire non-existent bid order returns ContractError::Unauthorized
        let expire_bid_msg = ExecuteMsg::ExpireAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

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
    fn expire_bid_sender_notequal() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        let exec_info = mock_info("not_exec", &[]);

        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: None,
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // expire bid order with sender not equal to owner returns ContractError::Unauthorized
        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

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
    fn expire_bid_with_sent_funds() {
        let mut deps = mock_provenance_dependencies();
        setup_test_base_contract_v3(&mut deps.storage);

        // expire bid order with sent_funds returns ContractError::ExpireWithFunds
        let exec_info = mock_info("exec_1", &coins(1, "sent_coin"));
        let expire_bid_msg = ExecuteMsg::ExpireAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

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

    #[test]
    fn expire_partial_filled_bid_with_fees_valid() {
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
                    rate: "0.003".to_string(),
                }),
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        // create bid data
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10000),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::new(2000),
                accumulated_quote: Uint128::new(20),
                accumulated_fee: Uint128::new(6),
                fee: Some(Coin {
                    amount: Uint128::new(30),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
                price: "0.01".into(),
            },
        );

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let expire_bid_msg = ExecuteMsg::ExpireBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let expire_bid_response = execute(deps.as_mut(), mock_env(), exec_info, expire_bid_msg);

        match expire_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "expire_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "8000")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(reject_bid_response.messages.len(), 2);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(80, "quote_1"),
                    })
                );
                assert_eq!(
                    reject_bid_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(24, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }
}
