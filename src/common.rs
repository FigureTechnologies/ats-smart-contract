use cosmwasm_std::{Addr, Coin, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeInfo {
    pub account: Addr,
    pub rate: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum Action {
    Fill {
        base: Coin,
        fee: Option<Coin>,
        price: String,
        quote: Coin,
    },
    Refund {
        fee: Option<Coin>,
        quote: Coin,
    },
    Reject {
        base: Coin,
        fee: Option<Coin>,
        quote: Coin,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Event {
    pub action: Action,
    pub block_info: BlockInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct BlockInfo {
    pub height: u64,
    pub time: Timestamp,
}

impl From<cosmwasm_std::BlockInfo> for BlockInfo {
    fn from(block_info: cosmwasm_std::BlockInfo) -> Self {
        BlockInfo {
            height: block_info.height,
            time: block_info.time,
        }
    }
}
