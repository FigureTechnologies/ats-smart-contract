use cosm_orc::orchestrator::Denom;
use serde::Serialize;
use uuid::Uuid;

// data class Coin(
//     val denom: String,
//     val amount: String
// )

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Coin {
    denom: String,
    amount: String,
}

// data class ExecuteRequest(
//     @JsonProperty("cancel_ask") val cancelAsk: IdRequest? = null,
//     @JsonProperty("cancel_bid") val cancelBid: IdRequest? = null,
//     @JsonProperty("create_ask") val createAsk: CreateAskRequest? = null,
//     @JsonProperty("create_bid") val createBid: CreateBidRequest? = null,
//     @JsonProperty("approve_ask") val approveAsk: ApproveAskRequest? = null,
//     @JsonProperty("reject_ask") val rejectAsk: RejectRequest? = null,
//     @JsonProperty("reject_bid") val rejectBid: RejectRequest? = null,
//     @JsonProperty("expire_ask") val expireAsk: IdRequest? = null,
//     @JsonProperty("expire_bid") val expireBid: IdRequest? = null,
//     @JsonProperty("execute_match") val executeMatch: ExecuteMatchRequest? = null,
//     @JsonProperty("modify_contract") val modifyContract: OrderBookModifyRequest? = null,
// )

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecuteRequest {
    CreateAsk {
        id: String, // UUID
        quote: String,
        price: String,
        base: String,
        size: String,
    },
    CreateBid {
        id: String, // uuid
        base: String,
        price: String,
        size: String,
        quote: String,
        quote_size: String,
        fee: Option<Coin>,
    },
    ExecuteMatch {
        bid_id: String, // uuid
        ask_id: String, // uuid
        price: String,
        size: String,
    },
}

pub(crate) fn create_bid(
    id: &Uuid,
    base_denom: &Denom,
    price: &str,
    size: &str,
    quote_denom: &Denom,
    quote_size: &str,
) -> ExecuteRequest {
    ExecuteRequest::CreateBid {
        id: id.to_string(),
        base: base_denom.to_string(),
        price: price.to_owned(),
        size: size.to_owned(),
        quote: quote_denom.to_string(),
        quote_size: quote_size.to_owned(),
        fee: None,
    }
}

pub(crate) fn create_ask(
    id: &Uuid,
    quote_denom: &Denom,
    price: &str,
    base_denom: &Denom,
    size: &str,
) -> ExecuteRequest {
    ExecuteRequest::CreateAsk {
        id: id.to_string(),
        quote: quote_denom.to_string(),
        price: price.to_owned(),
        base: base_denom.to_string(),
        size: size.to_owned(),
    }
}

pub(crate) fn execute_match(
    bid_id: &Uuid,
    ask_id: &Uuid,
    price: &str,
    size: &str,
) -> ExecuteRequest {
    ExecuteRequest::ExecuteMatch {
        bid_id: bid_id.to_string(),
        ask_id: ask_id.to_string(),
        price: price.to_owned(),
        size: size.to_owned(),
    }
}
