use crate::ask_order::{get_ask_storage, AskOrderV1};
use crate::bid_order::{get_bid_storage, BidOrderV2};
use crate::contract_info::{set_contract_info, ContractInfoV3};
use crate::tests::test_constants::{APPROVER_1, APPROVER_2, BASE_DENOM};
use cosmwasm_std::{Addr, Storage, Uint128};
use provwasm_mocks::ProvenanceMockQuerier;

pub fn setup_test_base(storage: &mut dyn Storage, contract_info: &ContractInfoV3) {
    if let Err(error) = set_contract_info(storage, contract_info) {
        panic!("unexpected error: {:?}", error)
    }
}

pub fn setup_test_base_contract_v3(storage: &mut dyn Storage) {
    setup_test_base(
        storage,
        &ContractInfoV3 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: BASE_DENOM.into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked(APPROVER_1), Addr::unchecked(APPROVER_2)],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            ask_fee_info: None,
            bid_fee_info: None,
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128::new(2),
            size_increment: Uint128::new(100),
        },
    );
}

pub fn store_test_ask(storage: &mut dyn Storage, ask_order: &AskOrderV1) {
    let mut ask_storage = get_ask_storage(storage);
    if let Err(error) = ask_storage.save(ask_order.id.as_bytes(), ask_order) {
        panic!("unexpected error: {:?}", error)
    };
}

pub fn store_test_bid(storage: &mut dyn Storage, bid_order: &BidOrderV2) {
    let mut bid_storage = get_bid_storage(storage);
    if let Err(error) = bid_storage.save(bid_order.id.as_bytes(), bid_order) {
        panic!("unexpected error: {:?}", error);
    };
}

pub fn set_default_required_attributes(
    querier: &mut ProvenanceMockQuerier,
    address: &str,
    ask_attributes: bool,
    bid_attributes: bool,
) {
    let mut attributes: Vec<(&str, &str, &str)> = Vec::new();
    if ask_attributes {
        attributes.append(&mut vec![
            ("ask_tag_1", "ask_tag_1_value", "String"),
            ("ask_tag_2", "ask_tag_2_value", "String"),
        ])
    }
    if bid_attributes {
        attributes.append(&mut vec![
            ("bid_tag_1", "bid_tag_1_value", "String"),
            ("bid_tag_2", "bid_tag_2_value", "String"),
        ])
    }
    querier.with_attributes(address, &attributes);
}
