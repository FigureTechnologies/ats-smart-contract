#[cfg(test)]
mod instantiate_tests {
    use crate::common::FeeInfo;
    use crate::contract::instantiate;
    use crate::contract_info::{get_contract_info, ContractInfoV3};
    use crate::error::ContractError;
    use crate::msg::InstantiateMsg;
    use crate::version_info::{get_version_info, VersionInfoV1, CRATE_NAME, PACKAGE_VERSION};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, Addr, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn instantiate_valid_data() {
        // create valid init data
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("contract_admin", &[]);
        let init_msg = InstantiateMsg {
            name: "contract_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec!["approver_1".into(), "approver_2".into()],
            executors: vec!["exec_1".into(), "exec_2".into()],
            ask_fee_rate: Some("0.01".into()),
            ask_fee_account: Some("ask_fee_account".into()),
            bid_fee_rate: Some("0.02".into()),
            bid_fee_account: Some("bid_fee_account".into()),
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(100),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg.clone());

        // verify initialize response
        match init_response {
            Ok(init_response) => {
                assert!(init_response.messages.is_empty());
                let expected_contract_info = ContractInfoV3 {
                    name: "contract_name".into(),
                    bind_name: "".into(),
                    base_denom: "base_denom".into(),
                    convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                    supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                    approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                    executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                    ask_fee_info: Some(FeeInfo {
                        account: Addr::unchecked("ask_fee_account"),
                        rate: "0.01".into(),
                    }),
                    bid_fee_info: Some(FeeInfo {
                        account: Addr::unchecked("bid_fee_account"),
                        rate: "0.02".into(),
                    }),
                    ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                    bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                    price_precision: Uint128::new(2),
                    size_increment: Uint128::new(100),
                };

                let expected_version_info = VersionInfoV1 {
                    definition: CRATE_NAME.to_string(),
                    version: PACKAGE_VERSION.to_string(),
                };

                assert_eq!(init_response.attributes.len(), 2);
                assert_eq!(
                    init_response.attributes[0],
                    attr("contract_info", format!("{:?}", expected_contract_info))
                );
                assert_eq!(init_response.attributes[1], attr("action", "init"));
                assert_eq!(
                    get_contract_info(&deps.storage).unwrap(),
                    expected_contract_info
                );
                assert!(init_response.messages.is_empty());
                assert_eq!(
                    get_version_info(&deps.storage).unwrap(),
                    expected_version_info
                );
            }
            error => panic!("failed to initialize: {:?}", error),
        }
    }

    #[test]
    fn instantiate_invalid_data() {
        // create invalid init data
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("contract_owner", &[]);
        let init_msg = InstantiateMsg {
            name: "".into(),
            base_denom: "".into(),
            convertible_base_denoms: vec![],
            supported_quote_denoms: vec![],
            approvers: vec![],
            executors: vec![],
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: vec![],
            bid_required_attributes: vec![],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(100),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg);

        // verify initialize response
        match init_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"name".into()));
                    assert!(fields.contains(&"base_denom".into()));
                    assert!(fields.contains(&"supported_quote_denoms".into()));
                    assert!(fields.contains(&"executors".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn instantiate_invalid_price_size_increment_pair() {
        // create invalid init data
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("contract_owner", &[]);
        let init_msg = InstantiateMsg {
            name: "contract_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec!["approver_1".into(), "approver_2".into()],
            executors: vec!["exec_1".into(), "exec_2".into()],
            ask_fee_rate: None,
            ask_fee_account: None,
            bid_fee_rate: None,
            bid_fee_account: None,
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(10),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg);

        // verify initialize response
        match init_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidPricePrecisionSizePair => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }
}
