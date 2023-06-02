#[cfg(test)]
mod create_bid_tests {
    use crate::bid_order::{get_bid_storage_read, BidOrderV3};
    use crate::common::FeeInfo;
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{
        BASE_DENOM, HYPHENATED_BID_ID, QUOTE_DENOM_1, UNHYPHENATED_BID_ID,
    };
    use crate::tests::test_setup_utils::{
        set_default_required_attributes, setup_test_base, setup_test_base_contract_v3,
    };
    use crate::tests::test_utils::validate_execute_invalid_id_field;
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{attr, coin, coins, from_binary, Addr, Binary, Coin, Uint128};
    use provwasm_mocks::mock_dependencies;
    use provwasm_std::{transfer_marker_coins, Marker};

    const QUOTE1_RESTRICTED_MARKER_JSON: &str = "{
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

    #[test]
    fn create_bid_valid_with_fee_less_than_one_but_wrong_denom_return_err() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: HYPHENATED_BID_ID.to_string(),
            base: BASE_DENOM.into(),
            fee: Some(coin(0, "BAD_DENOM")), // Incorrect fee denom
            price: "2.5".into(),
            quote: QUOTE_DENOM_1.into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };
        // Add bid required attributes
        set_default_required_attributes(&mut deps.querier, "bidder", false, true);

        let bidder_info = mock_info("bidder", &coins(250, QUOTE_DENOM_1));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(ContractError::SentFundsOrderMismatch) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }

    #[test]
    fn create_bid_valid_with_fee_less_than_one_is_accepted() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: HYPHENATED_BID_ID.to_string(),
            base: BASE_DENOM.into(),
            fee: Some(coin(0, QUOTE_DENOM_1)),
            price: "2.5".into(),
            quote: QUOTE_DENOM_1.into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };
        // Add bid required attributes
        set_default_required_attributes(&mut deps.querier, "bidder", false, true);

        let bidder_info = mock_info("bidder", &coins(250, QUOTE_DENOM_1));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", BASE_DENOM));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr("fee", format!("{:?}", coin(0, QUOTE_DENOM_1)))
                );
                assert_eq!(response.attributes[4], attr("price", "2.5"));
                assert_eq!(response.attributes[5], attr("quote", QUOTE_DENOM_1));
                assert_eq!(response.attributes[6], attr("quote_size", "250"));
                assert_eq!(response.attributes[7], attr("size", "100"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
            fee,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_valid_data_unhyphenated() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: UNHYPHENATED_BID_ID.to_string(), // Unhyphenated UUID
            base: "base_1".into(),
            fee: None,
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(250, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        validate_execute_invalid_id_field(create_bid_response)
    }

    #[test]
    fn create_bid_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_1".into(),
            fee: None,
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(250, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(response.attributes[3], attr("fee", "None"));
                assert_eq!(response.attributes[4], attr("price", "2.5"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "250"));
                assert_eq!(response.attributes[7], attr("size", "100"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
            fee,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_restricted_marker_valid_data() {
        let mut deps = mock_dependencies(&[]);
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

        let test_marker: Marker =
            from_binary(&Binary::from(QUOTE1_RESTRICTED_MARKER_JSON.as_bytes())).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            fee: None,
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(1000),
            size: Uint128::new(500),
        };

        let bidder_info = mock_info("bidder", &[]);

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(response.attributes[3], attr("fee", "None"));
                assert_eq!(response.attributes[4], attr("price", "2"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "1000"));
                assert_eq!(response.attributes[7], attr("size", "500"));

                assert_eq!(response.messages.len(), 1);
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        1000,
                        "quote_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("bidder")
                    )
                    .unwrap()
                );
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify bid order stored
        let ask_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
            fee,
            price,
            quote,
            quote_size,
            size,
        } = create_bid_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            id,
                            owner: bidder_info.sender,
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_fees_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".into(),
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            fee: Some(Coin {
                amount: Uint128::new(25),
                denom: "quote_1".into(),
            }),
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(275, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(25),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "2.5"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "250"));
                assert_eq!(response.attributes[7], attr("size", "100"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_fees_round_down_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_1".into(),
            fee: Some(Coin {
                amount: Uint128::new(1),
                denom: "quote_1".into(),
            }),
            price: "1".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(149),
            size: Uint128::new(149),
        };

        let bidder_info = mock_info("bidder", &coins(150, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(1),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "149"));
                assert_eq!(response.attributes[7], attr("size", "149"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
            fee,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_fees_round_up_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
            },
        );

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_1".into(),
            fee: Some(Coin {
                amount: Uint128::new(2),
                denom: "quote_1".into(),
            }),
            price: "1".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(150),
            size: Uint128::new(150),
        };

        let bidder_info = mock_info("bidder", &coins(152, "quote_1"));

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(2),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "150"));
                assert_eq!(response.attributes[7], attr("size", "150"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_restricted_marker_with_fees_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_acct"),
                    rate: "0.1".into(),
                }),
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

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".to_string(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            fee: Some(Coin {
                amount: Uint128::new(100),
                denom: "quote_1".into(),
            }),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(1000),
            size: Uint128::new(500),
        };

        let bidder_info = mock_info("bidder", &[]);

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );

        // verify create bid response
        match create_bid_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(100),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "2"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "1000"));
                assert_eq!(response.attributes[7], attr("size", "500"));

                assert_eq!(response.messages.len(), 1);
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        1100,
                        "quote_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("bidder")
                    )
                    .unwrap()
                );
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read::<BidOrderV3>(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
            price,
            quote,
            quote_size,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV3 {
                            id,
                            owner: bidder_info.sender,
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            accumulated_base: Uint128::zero(),
                            accumulated_quote: Uint128::zero(),
                            accumulated_fee: Uint128::zero(),
                            fee,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
                        }
                    )
                }
                _ => {
                    panic!("bid order was not found in storage")
                }
            }
        } else {
            panic!("bid_message is not a CreateBid type. this is bad.")
        }
    }

    #[test]
    fn create_bid_with_restricted_marker_with_funds() {
        let mut deps = mock_dependencies(&[]);
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

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".to_string(),
            fee: None,
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(10),
            size: Uint128::new(500),
        };

        let bidder_info = mock_info("bidder", &[coin(10, "quote_2")]);

        // execute create bid
        let create_bid_response = execute(deps.as_mut(), mock_env(), bidder_info, create_bid_msg);

        // verify create bid response
        match create_bid_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                create_bid_response
            ),
        }
    }

    #[test]
    fn create_bid_existing_id() {
        let mut deps = mock_dependencies(&[]);
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
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(250, "quote_1"));

        // execute create bid
        let create_bid_response = execute(deps.as_mut(), mock_env(), bidder_info, create_bid_msg);

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => {}
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // create bid data using existing id
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(900),
            size: Uint128::new(200),
        };

        let bidder_info = mock_info("bidder", &coins(900, "quote_1"));

        // execute create bid
        let create_bid_response = execute(deps.as_mut(), mock_env(), bidder_info, create_bid_msg);

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify bid order stored is the original order
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
                        accumulated_base: Uint128::zero(),
                        accumulated_quote: Uint128::zero(),
                        accumulated_fee: Uint128::zero(),
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2.5".into(),
                        quote: Coin {
                            amount: Uint128::new(250),
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
    fn create_bid_invalid_data() {
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

        // create bid missing id
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "".into(),
            fee: None,
            id: "".into(),
            price: "".into(),
            quote: "".into(),
            quote_size: Uint128::new(0),
            size: Uint128::new(0),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(100, "quote_1")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                    assert!(fields.contains(&"base".into()));
                    assert!(fields.contains(&"price".into()));
                    assert!(fields.contains(&"size".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_invalid_base() {
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

        // create bid with invalid base
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "notbasedenom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_2".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };

        // execute create ask
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(200, "quote_2")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InconvertibleBaseDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_unsupported_quote() {
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

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "unsupported".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(200, "unsupported")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedQuoteDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_sent_funds_not_equal_price_times_size() {
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
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(100),
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(100, "quote_1")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_wrong_account_attributes() {
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
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(200, "quote_1")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::Unauthorized => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_invalid_price_precision() {
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
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.123".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(200, "quote_1")),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
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
    fn create_bid_restricted_quote_denom_and_quote_mismatch_order_amount_and_size() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        let test_marker: Marker =
            from_binary(&Binary::from(QUOTE1_RESTRICTED_MARKER_JSON.as_bytes())).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "3".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200), // Valid amount would be 300
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &[]),
            create_bid_msg,
        );

        // verify execute create bid response
        match create_bid_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }
}
