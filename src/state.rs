use cosmwasm_std::{Coin, HumanAddr, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, Bucket, ReadonlyBucket};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static NAMESPACE_ORDER_ASK: &[u8] = b"ask";
pub static NAMESPACE_ORDER_BID: &[u8] = b"bid";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderStatus {
    Ready,
    PendingIssuerApproval,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderClass {
    Basic,
    Convertible { status: AskOrderStatus },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AskOrder {
    pub base: Coin,
    pub class: AskOrderClass,
    pub id: String,
    pub owner: HumanAddr,
    pub price: Uint128,
    pub quote: String,
    pub size: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BidOrder {
    pub base: String,
    pub id: String,
    pub owner: HumanAddr,
    pub price: Uint128,
    pub quote: Coin,
    pub size: Uint128,
}

pub fn get_ask_storage(storage: &mut dyn Storage) -> Bucket<AskOrder> {
    bucket(storage, NAMESPACE_ORDER_ASK)
}

pub fn get_ask_storage_read(storage: &dyn Storage) -> ReadonlyBucket<AskOrder> {
    bucket_read(storage, NAMESPACE_ORDER_ASK)
}
pub fn get_bid_storage(storage: &mut dyn Storage) -> Bucket<BidOrder> {
    bucket(storage, NAMESPACE_ORDER_BID)
}

pub fn get_bid_storage_read(storage: &dyn Storage) -> ReadonlyBucket<BidOrder> {
    bucket_read(storage, NAMESPACE_ORDER_BID)
}
