#[cfg(test)]
mod create_ask_tests {
    use crate::ask_order::{get_ask_storage_read, AskOrderClass, AskOrderV1};
    use crate::contract::execute;
    use crate::contract_info::ContractInfoV3;
    use crate::msg::ExecuteMsg;
    use crate::tests::test_constants::{HYPHENATED_ASK_ID, UNHYPHENATED_ASK_ID};
    use crate::tests::test_utils::setup_test_base;
    use cosmwasm_std::testing::{mock_env, mock_info};
    use cosmwasm_std::{attr, coins, Addr, Uint128};
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn create_ask_valid_data_unhyphenated_id() {
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
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        deps.querier.with_attributes(
            "asker",
            &[
                ("ask_tag_1", "ask_tag_1_value", "String"),
                ("ask_tag_2", "ask_tag_2_value", "String"),
            ],
        );

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
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(response.attributes[1], attr("id", HYPHENATED_ASK_ID));
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
            match ask_storage.load(HYPHENATED_ASK_ID.as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id: HYPHENATED_ASK_ID.into(),
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
}
