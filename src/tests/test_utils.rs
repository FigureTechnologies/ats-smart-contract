use crate::ask_order::{get_ask_storage, AskOrderV1};
use crate::bid_order::{get_bid_storage, BidOrderV2};
use crate::contract_info::{set_contract_info, ContractInfoV3};
use cosmwasm_std::Storage;

pub fn setup_test_base(storage: &mut dyn Storage, contract_info: &ContractInfoV3) {
    if let Err(error) = set_contract_info(storage, contract_info) {
        panic!("unexpected error: {:?}", error)
    }
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
