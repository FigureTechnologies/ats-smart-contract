#[cfg(test)]
mod execute_modify_test {
    use crate::ask_order::{AskOrderClass, AskOrderV1};
    use crate::common::FeeInfo;
    use crate::contract::execute;
    use crate::contract_info::{get_contract_info, ContractInfoV3};
    use crate::error::ContractError;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_setup_utils::{setup_test_base, store_test_ask};
    use crate::version_info::{set_version_info, VersionInfoV1};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{coin, Addr, MessageInfo, Uint128};
    use provwasm_mocks::mock_provenance_dependencies;
    use provwasm_std::types::provenance::attribute::v1::{
        Attribute, AttributeType, QueryAttributeRequest, QueryAttributeResponse,
    };

    #[test]
    fn execute_modify_contract_valid() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec1".into(), "exec3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_3".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec1"), Addr::unchecked("exec3")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("approver_1"), Addr::unchecked("approver_3")])
                );
                assert_eq!(
                    contract_info.ask_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_1"),
                        rate: "0.123".into()
                    })
                );
                assert_eq!(
                    contract_info.bid_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_2"),
                        rate: "0.234".into()
                    })
                );
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_3".to_string()]
                );
            }
        }
    }

    #[test]
    fn execute_modify_contract_invalid_executor() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.1".to_string(),
            },
        );
        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }
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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(contract_info.bid_fee_info, None);
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_2".to_string()]
                );
            }
        }
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec1".into(), "exec3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_3".into()]),
        };
        let exec_info = mock_info("invalid_exec", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(contract_info.bid_fee_info, None);
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_2".to_string()]
                );
            }
        }
    }

    #[test]
    fn execute_modify_contract_invalid_fields() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );
        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }
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
                ask_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("ask_fee_account"),
                    rate: "0.01".into(),
                }),
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: Some("0.0s".into()),
            bid_fee_account: Some("bid_fee_account".into()),
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_fee_rate".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: Some("0.01".into()),
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_fee_account".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: Some("bid_fee_account".into()),
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_fee_rate".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: Some("0.01".into()),
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_fee_account".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: Some("ask_fee_account".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_fee_rate".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: Some(vec![]),
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["executors_empty".to_string()])
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn execute_modify_contract_invalid_version() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.1".to_string(),
            },
        );
        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }
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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(contract_info.bid_fee_info, None);
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_2".to_string()]
                );
            }
        }
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec1".into(), "exec3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_3".into()]),
        };
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("unexpected because invalid version"),
            Err(error) => match error {
                ContractError::UnsupportedUpgrade {
                    source_version,
                    target_version,
                } => {
                    assert_eq!(source_version, "<0.16.2".to_string());
                    assert_eq!(target_version, ">=0.16.2".to_string());
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(contract_info.ask_fee_info, None);
                assert_eq!(contract_info.bid_fee_info, None);
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_2".to_string()]
                );
            }
        }
    }

    #[test]
    fn execute_modify_contract_invalid_attributes() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );
        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("approver_1"), Addr::unchecked("approver_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        // modify ask_required_attributes with no ask
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec!["ask_tag_1".into(), "ask_tag_2".into()]),
            bid_required_attributes: None,
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "asker".to_string(),
                attributes: vec![
                    Attribute {
                        name: "ask_tag_1".to_string(),
                        value: "ask_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "ask_tag_2".to_string(),
                        value: "ask_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let asker_info: MessageInfo = mock_info("asker", &[coin(100, "base_denom")]);
        let create_ask_msg = ExecuteMsg::CreateAsk {
            base: "base_denom".into(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );
        match create_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // modify ask_required_attributes with active ask
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec![
                "ask_tag_1".into(),
                "ask_tag_2".into(),
                "ask_tag_3".into(),
            ]),
            bid_required_attributes: None,
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "bidder".to_string(),
                attributes: vec![
                    Attribute {
                        name: "bid_tag_1".to_string(),
                        value: "bid_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "bid_tag_2".to_string(),
                        value: "bid_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let bidder_info: MessageInfo = mock_info("bidder", &[coin(200, "quote_1")]);
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "ab5f5a62-f6fc-46d1-aa84-61ccc51ec367".into(),
            base: "base_denom".into(),
            fee: None,
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );
        match create_bid_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // modify bid_required_attributes with active bid
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: Some(vec![
                "bid_tag_1".into(),
                "bid_tag_2".into(),
                "bid_tag_3".into(),
            ]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_2".to_string()]
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }

    #[test]
    fn execute_modify_contract_add_approvers() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );
        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("approver_1"), Addr::unchecked("approver_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "asker".to_string(),
                attributes: vec![
                    Attribute {
                        name: "ask_tag_1".to_string(),
                        value: "ask_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "ask_tag_2".to_string(),
                        value: "ask_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let asker_info: MessageInfo = mock_info("asker", &[coin(100, "base_denom")]);
        let create_ask_msg = ExecuteMsg::CreateAsk {
            base: "base_denom".into(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );
        match create_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // empty executors not allowed, else anyone can execute
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["approvers".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        // empty executors not allowed, else anyone can execute
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec![
                "approver_1".into(),
                "approver_2".into(),
                "approver_3".into(),
            ]),
            executors: None,
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([
                        Addr::unchecked("approver_1"),
                        Addr::unchecked("approver_2"),
                        Addr::unchecked("approver_3")
                    ])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }
    }

    #[test]
    fn execute_modify_contract_invalid_input_executors() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        let contract_info = get_contract_info(&deps.storage);

        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        // empty executors not allowed, else anyone can execute
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec![]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec!["ask_tag_3".into(), "ask_tag_4".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["executors_empty".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        // empty approvers not allowed, else anyone cn approve
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec![]),
            executors: None,
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec!["ask_tag_3".into(), "ask_tag_4".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["approvers_empty".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: None,
            executors: None,
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }

    #[test]
    fn execute_modify_contract_invalid_attributes_conflict() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "asker".to_string(),
                attributes: vec![
                    Attribute {
                        name: "ask_tag_1".to_string(),
                        value: "ask_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "ask_tag_2".to_string(),
                        value: "ask_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let asker_info: MessageInfo = mock_info("asker", &[coin(100, "base_denom")]);
        let create_ask_msg = ExecuteMsg::CreateAsk {
            base: "base_denom".into(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );
        match create_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        // ask_required_attributes conflict with active asks
        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec!["ask_tag_1".into(), "ask_tag_2".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: Some(vec![
                "ask_tag_1".into(),
                "ask_tag_2".into(),
                "ask_tag_3".into(),
            ]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_2".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["ask_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let cancel_info: MessageInfo = mock_info("asker", &[]);
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            cancel_info.clone(),
            cancel_ask_msg,
        );
        match cancel_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec1".into(), "exec3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_3".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec1"), Addr::unchecked("exec3")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("approver_1"), Addr::unchecked("approver_3")])
                );
                assert_eq!(
                    contract_info.ask_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_1"),
                        rate: "0.123".into()
                    })
                );
                assert_eq!(
                    contract_info.bid_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_2"),
                        rate: "0.234".into()
                    })
                );
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_3".to_string()]
                );
            }
        }
    }

    #[test]
    fn execute_modify_contract_valid_remove_attributes() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: None,
            bid_required_attributes: Some(vec![]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string(), "ask_tag_2".to_string()]
                );
                let empty_vector: Vec<String> = vec![];
                assert_eq!(contract_info.bid_required_attributes, empty_vector);
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.ask_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_1"),
                        rate: "0.123".to_string(),
                    })
                );
            }
        }
    }

    #[test]
    fn execute_modify_contract_invalid_conflicting_bid_attributes() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }
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
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "asker".to_string(),
                attributes: vec![
                    Attribute {
                        name: "ask_tag_1".to_string(),
                        value: "ask_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "ask_tag_2".to_string(),
                        value: "ask_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let asker_info: MessageInfo = mock_info("asker", &[coin(100, "base_denom")]);
        let create_ask_msg = ExecuteMsg::CreateAsk {
            base: "base_denom".into(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );
        match create_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let create_ask_msg = ExecuteMsg::CreateAsk {
            base: "base_denom".into(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec368".into(),
            price: "3".into(),
            quote: "quote_2".into(),
            size: Uint128::new(100),
        };
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );
        match create_ask_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "bidder".to_string(),
                attributes: vec![
                    Attribute {
                        name: "bid_tag_1".to_string(),
                        value: "bid_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "bid_tag_2".to_string(),
                        value: "bid_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let bidder_info = mock_info("bidder", &[coin(200, "quote_1")]);
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec468".into(),
            base: "base_denom".to_string(),
            fee: None,
            price: "2".into(),
            quote: "quote_1".to_string(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );
        match create_bid_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: None,
            bid_required_attributes: Some(vec!["bid_tag_3".into(), "bid_tag_4".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn execute_modify_contract_invalid_conflicting_bid_fee() {
        let mut deps = mock_provenance_dependencies();

        let version_info = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.16.2".to_string(),
            },
        );

        match version_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => {}
        }

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
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec_1"), Addr::unchecked("exec_2")])
                );
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(contract_info.ask_fee_info, None);
            }
        }

        QueryAttributeRequest::mock_response(
            &mut deps.querier,
            QueryAttributeResponse {
                account: "bidder".to_string(),
                attributes: vec![
                    Attribute {
                        name: "bid_tag_1".to_string(),
                        value: "bid_tag_1_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "bid_tag_2".to_string(),
                        value: "bid_tag_2_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "bid_tag_3".to_string(),
                        value: "bid_tag_3_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                    Attribute {
                        name: "bid_tag_4".to_string(),
                        value: "bid_tag_4_value".as_bytes().to_vec(),
                        attribute_type: AttributeType::String.into(),
                        address: "".to_string(),
                    },
                ],
                pagination: None,
            },
        );

        let bidder_info = mock_info("bidder", &[coin(200, "quote_1")]);
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec468".into(),
            base: "base_denom".to_string(),
            fee: None,
            price: "2".into(),
            quote: "quote_1".to_string(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
        };
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            create_bid_msg.clone(),
        );
        match create_bid_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_1".into()),
            ask_required_attributes: None,
            bid_required_attributes: Some(vec![
                "bid_tag_1".into(),
                "bid_tag_2".into(),
                "bid_tag_3".into(),
            ]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_required_attributes".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec_1".into(), "exec_3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_1".into()),
            ask_required_attributes: None,
            bid_required_attributes: None,
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => panic!("expected modifyContract validation to fail"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert_eq!(fields, vec!["bid_fee".to_string()]);
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        let cancel_info: MessageInfo = mock_info("bidder", &[]);
        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec468".into(),
        };
        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            cancel_info.clone(),
            cancel_bid_msg,
        );
        match cancel_bid_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let exec_info = mock_info("exec_1", &[]);
        let modify_contract_msg = ExecuteMsg::ModifyContract {
            approvers: Some(vec!["approver_1".into(), "approver_3".into()]),
            executors: Some(vec!["exec1".into(), "exec3".into()]),
            ask_fee_rate: Some("0.123".into()),
            ask_fee_account: Some("fee_acct_1".into()),
            bid_fee_rate: Some("0.234".into()),
            bid_fee_account: Some("fee_acct_2".into()),
            ask_required_attributes: Some(vec!["ask_tag_1".into()]),
            bid_required_attributes: Some(vec!["bid_tag_1".into(), "bid_tag_3".into()]),
        };
        let modify_contract_response =
            execute(deps.as_mut(), mock_env(), exec_info, modify_contract_msg);
        match modify_contract_response {
            Ok(_) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(contract_info) => {
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1".to_string(), "con_base_2".to_string()]
                );
                assert_eq!(
                    contract_info.executors,
                    Vec::from([Addr::unchecked("exec1"), Addr::unchecked("exec3")])
                );
                assert_eq!(
                    contract_info.approvers,
                    Vec::from([Addr::unchecked("approver_1"), Addr::unchecked("approver_3")])
                );
                assert_eq!(
                    contract_info.ask_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_1"),
                        rate: "0.123".into()
                    })
                );
                assert_eq!(
                    contract_info.bid_fee_info,
                    Some(FeeInfo {
                        account: Addr::unchecked("fee_acct_2"),
                        rate: "0.234".into()
                    })
                );
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1".to_string()]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["bid_tag_1".to_string(), "bid_tag_3".to_string()]
                );
            }
        }
    }
}
