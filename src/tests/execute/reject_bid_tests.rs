#[cfg(test)]
mod reject_bid_tests {
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
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, coins, Addr, BankMsg, Coin, CosmosMsg, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn reject_bid_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: HYPHENATED_BID_ID.into(),
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

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: HYPHENATED_BID_ID.to_string(),
            size: None,
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", HYPHENATED_BID_ID)
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(reject_bid_response.messages.len(), 1);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
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
    fn reject_bid_legacy_unhyphenated_id_then_rejects_bid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        store_test_bid(
            &mut deps.storage,
            &BidOrderV3 {
                id: UNHYPHENATED_BID_ID.into(),
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

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: UNHYPHENATED_BID_ID.to_string(),
            size: None,
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", UNHYPHENATED_BID_ID)
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "false")
                );
                assert_eq!(reject_bid_response.messages.len(), 1);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
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
    fn reject_partial_bid_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
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
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(100)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "100")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "true")
                );
                assert_eq!(reject_bid_response.messages.len(), 1);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order update
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(200),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(100),
                        accumulated_quote: Uint128::new(200),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        quote: Coin {
                            amount: Uint128::new(400),
                            denom: "quote_1".into(),
                        },
                        price: "2".into(),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn reject_partial_bid_with_fees_valid() {
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".to_string(),
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
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(10),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(50)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "50")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "true")
                );
                assert_eq!(reject_bid_response.messages.len(), 2);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(50, "quote_1"),
                    })
                );
                assert_eq!(
                    reject_bid_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(5, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order update
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(100 - 50),
                        accumulated_quote: Uint128::new(100 - 50),
                        accumulated_fee: Uint128::new(10 - 5),
                        fee: Some(Coin {
                            amount: Uint128::new(10),
                            denom: "quote_1".to_string(),
                        }),
                        quote: Coin {
                            amount: Uint128::new(100),
                            denom: "quote_1".into(),
                        },
                        price: "1".into(),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn reject_partial_bid_with_fees_round_down() {
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".to_string(),
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
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(10),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(55)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "55")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "true")
                );
                assert_eq!(reject_bid_response.messages.len(), 2);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(55, "quote_1"),
                    })
                );
                assert_eq!(
                    reject_bid_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(5, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order update
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(55),
                        accumulated_quote: Uint128::new(55),
                        accumulated_fee: Uint128::new(5),
                        fee: Some(Coin {
                            amount: Uint128::new(10),
                            denom: "quote_1".to_string(),
                        }),
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        quote: Coin {
                            amount: Uint128::new(100),
                            denom: "quote_1".into(),
                        },
                        price: "1".into(),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn reject_partial_bid_with_fees_round_up() {
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".to_string(),
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
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
                fee: Some(Coin {
                    amount: Uint128::new(10),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // expire bid order
        let exec_info = mock_info("exec_1", &[]);

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(56)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(reject_bid_response) => {
                assert_eq!(reject_bid_response.attributes.len(), 4);
                assert_eq!(
                    reject_bid_response.attributes[0],
                    attr("action", "reject_bid")
                );
                assert_eq!(
                    reject_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    reject_bid_response.attributes[2],
                    attr("reverse_size", "56")
                );
                assert_eq!(
                    reject_bid_response.attributes[3],
                    attr("order_open", "true")
                );
                assert_eq!(reject_bid_response.messages.len(), 2);
                assert_eq!(
                    reject_bid_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(56, "quote_1"),
                    })
                );
                assert_eq!(
                    reject_bid_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".to_string(),
                        amount: coins(6, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order update
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        accumulated_base: Uint128::new(56),
                        accumulated_quote: Uint128::new(56),
                        accumulated_fee: Uint128::new(6),
                        fee: Some(Coin {
                            amount: Uint128::new(10),
                            denom: "quote_1".to_string(),
                        }),
                        quote: Coin {
                            amount: Uint128::new(100),
                            denom: "quote_1".into(),
                        },
                        price: "1".into(),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn reject_partial_bid_cancel_size_not_increment() {
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

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(50)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"size".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify bid order not updated
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
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
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn reject_bid_cancel_size_greater_than_order_size() {
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

        let reject_bid_msg = ExecuteMsg::RejectBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
            size: Some(Uint128::new(150)),
        };

        let reject_bid_response = execute(deps.as_mut(), mock_env(), exec_info, reject_bid_msg);

        match reject_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"size".into()))
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify bid order not updated
        match BIDS_V3.load(
            &deps.storage,
            "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes(),
        ) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV3 {
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
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }
}
