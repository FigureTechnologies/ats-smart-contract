#[cfg(test)]
mod reject_ask_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderV1};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_utils::{setup_test_base, store_test_ask};
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, coins, Addr, BankMsg, CosmosMsg, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn reject_ask_valid_unhyphenated_id() {
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
}
