use cosmwasm_std::{
    attr, coin, coins, entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps,
    DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use provwasm_std::{
    bind_name, Marker, MarkerMsgParams, MarkerType, NameBinding, ProvenanceMsg, ProvenanceQuerier,
    ProvenanceRoute,
};

use crate::ask_order::{
    get_ask_storage, get_ask_storage_read, migrate_ask_orders, AskOrderClass, AskOrderStatus,
    AskOrderV1,
};
use crate::bid_order::{get_bid_storage, get_bid_storage_read, migrate_bid_orders, BidOrderV1};
use crate::contract_info::{
    get_contract_info, migrate_contract_info, set_contract_info, ContractInfoV1,
};
use crate::error::ContractError;
use crate::error::ContractError::InvalidPricePrecisionSizePair;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Validate};
use crate::version_info::{get_version_info, migrate_version_info};
use rust_decimal::prelude::{FromStr, ToPrimitive, Zero};
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::Mul;

// smart contract initialization entrypoint
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    msg.validate()?;

    // Validate and convert approvers to addresses
    let mut approvers: Vec<Addr> = Vec::new();
    for approver_str in msg.approvers {
        let address = deps.api.addr_validate(&approver_str)?;
        approvers.push(address);
    }

    // Validate and convert executors to addresses
    let mut executors: Vec<Addr> = Vec::new();
    for executor_str in msg.executors {
        let address = deps.api.addr_validate(&executor_str)?;
        executors.push(address);
    }

    // set contract info
    let contract_info = ContractInfoV1 {
        name: msg.name,
        bind_name: msg.bind_name,
        base_denom: msg.base_denom,
        convertible_base_denoms: msg.convertible_base_denoms,
        supported_quote_denoms: msg.supported_quote_denoms,
        approvers,
        executors,
        ask_required_attributes: msg.ask_required_attributes,
        bid_required_attributes: msg.bid_required_attributes,
        price_precision: msg.price_precision,
        size_increment: msg.size_increment,
    };

    if (msg.size_increment.u128() % 10u128.pow(msg.price_precision.u128() as u32)).ne(&0) {
        return Err(InvalidPricePrecisionSizePair);
    }

    set_contract_info(deps.storage, &contract_info)?;

    // create name binding provenance message
    let bind_name_msg = bind_name(
        contract_info.bind_name,
        env.contract.address,
        NameBinding::Restricted,
    )?;

    // build response
    Ok(Response {
        submessages: vec![],
        messages: vec![bind_name_msg],
        attributes: vec![
            attr(
                "contract_info",
                format!("{:?}", get_contract_info(deps.storage)?),
            ),
            attr("action", "init"),
        ],
        data: None,
    })
}

// smart contract execute entrypoint
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // validate execute message
    msg.validate()?;

    match msg {
        ExecuteMsg::ApproveAsk { id, base, size } => approve_ask(deps, env, info, id, base, size),
        ExecuteMsg::CreateAsk {
            id,
            base,
            quote,
            price,
            size,
        } => create_ask(
            deps,
            env,
            &info,
            AskOrderV1 {
                base,
                class: AskOrderClass::Basic,
                id,
                owner: info.sender.to_owned(),
                quote,
                price,
                size,
            },
        ),
        ExecuteMsg::CreateBid {
            id,
            base,
            price,
            quote,
            quote_size,
            size,
        } => create_bid(
            deps,
            env,
            &info,
            BidOrderV1 {
                base,
                id,
                owner: info.sender.to_owned(),
                price,
                quote,
                quote_size,
                size,
            },
        ),
        ExecuteMsg::CancelAsk { id } => cancel_ask(deps, info, id),
        ExecuteMsg::CancelBid { id } => cancel_bid(deps, env, info, id),
        ExecuteMsg::ExecuteMatch {
            ask_id,
            bid_id,
            price,
            size,
        } => execute_match(deps, env, info, ask_id, bid_id, price, size),
    }
}

fn approve_ask(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
    base: String,
    size: Uint128,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    if !contract_info.approvers.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    // is ask base a marker
    let is_base_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(base.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // determine sent funds requirements
    match is_base_restricted_marker {
        // no funds should be sent if base is a restricted marker
        true => {
            if !info.funds.is_empty() {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
        // sent funds must match order if not a restricted marker
        false => {
            if info.funds.ne(&coins(size.into(), base.to_owned())) {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
    }

    let mut ask_storage = get_ask_storage(deps.storage);

    // update ask order
    let updated_ask_order = ask_storage.update(
        id.as_bytes(),
        |stored_ask_order| -> Result<AskOrderV1, ContractError> {
            match stored_ask_order {
                None => Err(ContractError::InvalidFields {
                    fields: vec![String::from("id")],
                }),
                Some(mut stored_ask_order) => {
                    if size.ne(&stored_ask_order.size) || base.ne(&contract_info.base_denom) {
                        return Err(ContractError::SentFundsOrderMismatch);
                    }

                    stored_ask_order.class = AskOrderClass::Convertible {
                        status: AskOrderStatus::Ready {
                            approver: info.sender.clone(),
                            converted_base: coin(size.into(), base.clone()),
                        },
                    };

                    Ok(stored_ask_order)
                }
            }
        },
    )?;

    // build response
    Ok(Response {
        submessages: vec![],
        messages: match is_base_restricted_marker {
            true => {
                vec![CosmosMsg::Custom(ProvenanceMsg {
                    route: ProvenanceRoute::Marker,
                    params: provwasm_std::ProvenanceMsgParams::Marker(
                        MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(size.into(), base),
                            to: env.contract.address,
                            from: info.sender,
                        },
                    ),
                    version: "2_0_0".to_string(),
                })]
            }
            false => {
                vec![]
            }
        },
        attributes: vec![
            attr("action", "approve_ask"),
            attr("id", &updated_ask_order.id),
            attr("class", serde_json::to_string(&updated_ask_order.class)?),
            attr("quote", &updated_ask_order.quote),
            attr("price", &updated_ask_order.price),
            attr("size", &updated_ask_order.size),
        ],
        data: None,
    })
}

// create ask entrypoint
fn create_ask(
    deps: DepsMut,
    env: Env,
    info: &MessageInfo,
    mut ask_order: AskOrderV1,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    // error if order base is not contract base nor contract convertible base
    if ask_order.base.ne(&contract_info.base_denom)
        && !contract_info
            .convertible_base_denoms
            .contains(&ask_order.base)
    {
        return Err(ContractError::InconvertibleBaseDenom);
    }

    // is ask base a marker
    let is_base_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(ask_order.base.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // determine sent funds requirements
    match is_base_restricted_marker {
        // no funds should be sent if base is a restricted marker
        true => {
            if !info.funds.is_empty() {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
        // sent funds must match order if not a restricted marker
        false => {
            if info
                .funds
                .ne(&coins(ask_order.size.into(), ask_order.base.to_owned()))
            {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
    }

    // error if quote denom unsupported
    if !contract_info
        .supported_quote_denoms
        .contains(&ask_order.quote)
    {
        return Err(ContractError::UnsupportedQuoteDenom);
    }

    // error if order size is not multiple of size_increment
    if (ask_order.size.u128() % contract_info.size_increment.u128()).ne(&0) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    let ask_price =
        Decimal::from_str(&ask_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("price")],
        })?;

    // error if price smaller than allow price precision
    if ask_price
        .mul(Decimal::from(
            10u128.pow(contract_info.price_precision.u128() as u32),
        ))
        .fract()
        .ne(&Decimal::zero())
    {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

    // error if asker does not have required account attributes
    if !contract_info.ask_required_attributes.is_empty() {
        let querier = ProvenanceQuerier::new(&deps.querier);
        let none: Option<String> = None;
        let attributes_container = querier.get_attributes(info.sender.clone(), none)?;
        let attributes_names: HashSet<String> = attributes_container
            .attributes
            .into_iter()
            .map(|item| item.name)
            .collect();
        if contract_info
            .ask_required_attributes
            .iter()
            .any(|item| !attributes_names.contains(item))
        {
            return Err(ContractError::Unauthorized);
        }
    }

    let mut ask_storage = get_ask_storage(deps.storage);

    if ask_order.base.ne(&contract_info.base_denom) {
        ask_order.class = AskOrderClass::Convertible {
            status: AskOrderStatus::PendingIssuerApproval,
        };
    };

    if ask_storage.may_load(ask_order.id.as_bytes())?.is_some() {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("id")],
        });
    }

    ask_storage.save(&ask_order.id.as_bytes(), &ask_order)?;

    Ok(Response {
        submessages: vec![],
        messages: match is_base_restricted_marker {
            true => {
                vec![CosmosMsg::Custom(ProvenanceMsg {
                    route: ProvenanceRoute::Marker,
                    params: provwasm_std::ProvenanceMsgParams::Marker(
                        MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(ask_order.size.into(), ask_order.base.to_owned()),
                            to: env.contract.address,
                            from: ask_order.owner,
                        },
                    ),
                    version: "2_0_0".to_string(),
                })]
            }
            false => {
                vec![]
            }
        },
        attributes: vec![
            attr("action", "create_ask"),
            attr("id", &ask_order.id),
            attr("class", serde_json::to_string(&ask_order.class)?),
            attr("target_base", &contract_info.base_denom),
            attr("base", &ask_order.base),
            attr("quote", &ask_order.quote),
            attr("price", &ask_order.price),
            attr("size", &ask_order.size),
        ],
        data: None,
    })
}

// create bid entrypoint
fn create_bid(
    deps: DepsMut,
    env: Env,
    info: &MessageInfo,
    bid_order: BidOrderV1,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    let bid_price =
        Decimal::from_str(&bid_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("price")],
        })?;

    // error if price smaller than allow price precision
    if bid_price
        .mul(Decimal::from(
            10u128.pow(contract_info.price_precision.u128() as u32),
        ))
        .fract()
        .ne(&Decimal::zero())
    {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

    // error if order size is not multiple of size_increment
    if (bid_order.size.u128() % contract_info.size_increment.u128()).ne(&0) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    // calculate quote total (price * size), error if overflows
    let total = bid_price
        .checked_mul(Decimal::from(bid_order.size.u128()))
        .ok_or(ContractError::TotalOverflow)?;

    // error if total is not an integer
    if total.fract().ne(&Decimal::zero()) {
        return Err(ContractError::NonIntegerTotal);
    }

    // error if total is not equal to sent funds
    if bid_order.quote_size.u128().ne(&total.to_u128().unwrap()) {
        return Err(ContractError::SentFundsOrderMismatch);
    }

    // error if order quote is not supported quote denom
    if !&contract_info
        .supported_quote_denoms
        .contains(&bid_order.quote)
    {
        return Err(ContractError::UnsupportedQuoteDenom);
    }

    // error if order base denom not equal to contract base denom
    if bid_order.base.ne(&contract_info.base_denom) {
        return Err(ContractError::InconvertibleBaseDenom);
    }

    // error if bidder does not have required account attributes
    if !contract_info.bid_required_attributes.is_empty() {
        let querier = ProvenanceQuerier::new(&deps.querier);
        let none: Option<String> = None;
        let attributes_container = querier.get_attributes(info.sender.clone(), none)?;
        let attributes_names: HashSet<String> = attributes_container
            .attributes
            .into_iter()
            .map(|item| item.name)
            .collect();
        if contract_info
            .bid_required_attributes
            .iter()
            .any(|item| !attributes_names.contains(item))
        {
            return Err(ContractError::Unauthorized);
        }
    }

    // is ask base a marker
    let is_quote_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(bid_order.quote.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // determine sent funds requirements
    match is_quote_restricted_marker {
        // no funds should be sent if base is a restricted marker
        true => {
            if !info.funds.is_empty() {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
        // sent funds must match order if not a restricted marker
        false => {
            if info.funds.ne(&coins(
                bid_order.quote_size.u128(),
                bid_order.quote.to_owned(),
            )) {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
    }

    let mut bid_storage = get_bid_storage(deps.storage);

    if bid_storage.may_load(bid_order.id.as_bytes())?.is_some() {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("id")],
        });
    }

    bid_storage.save(&bid_order.id.as_bytes(), &bid_order)?;

    Ok(Response {
        submessages: vec![],
        messages: match is_quote_restricted_marker {
            true => {
                vec![CosmosMsg::Custom(ProvenanceMsg {
                    route: ProvenanceRoute::Marker,
                    params: provwasm_std::ProvenanceMsgParams::Marker(
                        MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(bid_order.quote_size.into(), bid_order.quote.to_owned()),
                            to: env.contract.address,
                            from: bid_order.owner,
                        },
                    ),
                    version: "2_0_0".to_string(),
                })]
            }
            false => {
                vec![]
            }
        },
        attributes: vec![
            attr("action", "create_bid"),
            attr("id", &bid_order.id),
            attr("base", &bid_order.base),
            attr("quote", &bid_order.quote),
            attr("quote_size", &bid_order.quote_size),
            attr("price", &bid_order.price),
            attr("size", &bid_order.size),
        ],
        data: None,
    })
}

// cancel ask entrypoint
fn cancel_ask(
    deps: DepsMut,
    info: MessageInfo,
    id: String,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // return error if id is empty
    if id.is_empty() {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::CancelWithFunds);
    }

    let ask_storage = get_ask_storage_read(deps.storage);
    let AskOrderV1 {
        id,
        owner,
        class,
        base,
        size,
        ..
    } = ask_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;
    if !info.sender.eq(&owner) {
        return Err(ContractError::Unauthorized);
    }

    // remove the ask order from storage
    let mut ask_storage = get_ask_storage(deps.storage);
    ask_storage.remove(id.as_bytes());

    // return 'base' to owner, return converted_base to issuer if applicable

    let mut response = Response {
        submessages: vec![],
        messages: vec![BankMsg::Send {
            to_address: owner.to_string(),
            amount: coins(u128::from(size), base),
        }
        .into()],
        attributes: vec![attr("action", "cancel_ask"), attr("id", id)],
        data: None,
    };

    if let AskOrderClass::Convertible {
        status: AskOrderStatus::Ready {
            approver,
            converted_base,
        },
    } = class
    {
        response.messages.push(
            BankMsg::Send {
                to_address: approver.to_string(),
                amount: vec![converted_base],
            }
            .into(),
        );
    }

    Ok(response)
}

// cancel bid entrypoint
fn cancel_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // return error if id is empty
    if id.is_empty() {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::CancelWithFunds);
    }

    let bid_storage = get_bid_storage_read(deps.storage);
    let BidOrderV1 {
        id,
        owner,
        quote,
        quote_size,
        ..
    } = bid_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;
    if !info.sender.eq(&owner) {
        return Err(ContractError::Unauthorized);
    }

    // remove the ask order from storage
    let mut bid_storage = get_bid_storage(deps.storage);
    bid_storage.remove(id.as_bytes());

    // is bid quote a marker
    let is_quote_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(quote.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // 'send quote back to owner' message
    let response = Response {
        submessages: vec![],
        messages: match is_quote_restricted_marker {
            true => {
                vec![CosmosMsg::Custom(ProvenanceMsg {
                    route: ProvenanceRoute::Marker,
                    params: provwasm_std::ProvenanceMsgParams::Marker(
                        MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(quote_size.into(), quote.to_owned()),
                            to: owner,
                            from: env.contract.address,
                        },
                    ),
                    version: "2_0_0".to_string(),
                })]
            }
            false => {
                vec![BankMsg::Send {
                    to_address: owner.to_string(),
                    amount: vec![coin(quote_size.u128(), quote)],
                }
                .into()]
            }
        },

        attributes: vec![attr("action", "cancel_bid"), attr("id", id)],
        data: None,
    };

    Ok(response)
}

// match and execute an ask and bid order
fn execute_match(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ask_id: String,
    bid_id: String,
    price: String,
    execute_size: Uint128,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // only executors may execute matches
    if !get_contract_info(deps.storage)?
        .executors
        .contains(&info.sender)
    {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::ExecuteWithFunds);
    }

    let ask_storage_read = get_ask_storage_read(deps.storage);
    let mut ask_order = ask_storage_read
        .load(ask_id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    let bid_storage_read = get_bid_storage_read(deps.storage);
    let mut bid_order = bid_storage_read
        .load(bid_id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    let ask_price =
        Decimal::from_str(&ask_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("AskOrder.price")],
        })?;
    let bid_price =
        Decimal::from_str(&bid_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("BidOrder.price")],
        })?;
    let execute_price = Decimal::from_str(&price).map_err(|_| ContractError::InvalidFields {
        fields: vec![String::from("ExecuteMsg.price")],
    })?;

    match ask_price.cmp(&bid_price) {
        // order prices overlap, use ask or bid price determined by execute msg provided price
        Ordering::Less => {
            if execute_price.ne(&ask_price) && execute_price.ne(&bid_price) {
                return Err(ContractError::InvalidExecutePrice);
            }
        }
        // if order prices are equal, execute price should be equal
        Ordering::Equal => {
            if execute_price.ne(&ask_price) {
                return Err(ContractError::InvalidExecutePrice);
            }
        }
        // ask price is greater than bid price, normal price spread behavior and should not match
        Ordering::Greater => {
            return Err(ContractError::AskBidPriceMismatch);
        }
    }

    // at least one side of the order will always execute fully, both sides if order sizes equal
    // so the provided execute match size must be either the ask or bid size (or both if equal)
    if execute_size.gt(&ask_order.size) || execute_size.gt(&bid_order.size) {
        return Err(ContractError::InvalidExecuteSize);
    }

    // calculate total (price * size), error if overflows
    let total = execute_price
        .checked_mul(Decimal::from(execute_size.u128()))
        .ok_or(ContractError::TotalOverflow)?;

    // error if total is not an integer
    if total.fract().ne(&Decimal::zero()) {
        return Err(ContractError::NonIntegerTotal);
    }

    let quote_total = Uint128(total.to_u128().ok_or(ContractError::TotalOverflow)?);

    ask_order.size = Uint128(ask_order.size.u128() - execute_size.u128());

    let ask_order_class = &mut ask_order.class;

    if let AskOrderClass::Convertible {
        status: AskOrderStatus::Ready { converted_base, .. },
    } = ask_order_class
    {
        converted_base.amount = ask_order.size;
    }

    bid_order.size = Uint128(bid_order.size.u128() - execute_size.u128());
    bid_order.quote_size = Uint128(bid_order.quote_size.u128() - quote_total.u128());

    // calculate refund to bidder if bid order is completed but quote funds remain
    let mut bidder_refund = Uint128(0);
    if bid_order.size.is_zero() && !bid_order.quote_size.is_zero() {
        bidder_refund = bid_order.quote_size;
        bid_order.quote_size = Uint128(bid_order.quote_size.u128() - bidder_refund.u128());
    }

    // is ask base a restricted marker
    let is_base_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(ask_order.base.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // is bid quote a restricted marker
    let is_quote_restricted_marker = matches!(
        ProvenanceQuerier::new(&deps.querier).get_marker_by_denom(bid_order.quote.clone()),
        Ok(Marker {
            marker_type: MarkerType::Restricted,
            ..
        })
    );

    // 'send quote to asker' and 'send base to bidder' messages
    let mut response = match &ask_order_class {
        AskOrderClass::Basic => Response {
            submessages: vec![],
            messages: vec![
                match is_quote_restricted_marker {
                    true => CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: provwasm_std::ProvenanceMsgParams::Marker(
                            MarkerMsgParams::TransferMarkerCoins {
                                coin: coin(quote_total.u128(), bid_order.quote.to_owned()),
                                to: ask_order.owner.to_owned(),
                                from: env.contract.address.to_owned(),
                            },
                        ),
                        version: "2_0_0".to_string(),
                    }),
                    false => BankMsg::Send {
                        to_address: ask_order.owner.to_string(),
                        amount: vec![Coin {
                            denom: bid_order.quote.to_owned(),
                            amount: quote_total,
                        }],
                    }
                    .into(),
                },
                match is_base_restricted_marker {
                    true => CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: provwasm_std::ProvenanceMsgParams::Marker(
                            MarkerMsgParams::TransferMarkerCoins {
                                coin: coin(execute_size.into(), ask_order.base.to_owned()),
                                to: bid_order.owner.to_owned(),
                                from: env.contract.address,
                            },
                        ),
                        version: "2_0_0".to_string(),
                    }),
                    false => BankMsg::Send {
                        to_address: bid_order.owner.to_string(),
                        amount: vec![Coin {
                            denom: ask_order.base.clone(),
                            amount: execute_size,
                        }],
                    }
                    .into(),
                },
            ],
            attributes: vec![
                attr("action", "execute"),
                attr("ask_id", &ask_id),
                attr("bid_id", &bid_id),
                attr("base", &bid_order.base),
                attr("quote", &ask_order.quote),
                attr("price", &execute_price),
                attr("size", &execute_size),
            ],
            data: None,
        },
        AskOrderClass::Convertible {
            status:
                AskOrderStatus::Ready {
                    approver,
                    converted_base,
                },
        } => Response {
            submessages: vec![],
            messages: vec![
                match is_base_restricted_marker {
                    true => CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: provwasm_std::ProvenanceMsgParams::Marker(
                            MarkerMsgParams::TransferMarkerCoins {
                                coin: coin(execute_size.into(), ask_order.base.to_owned()),
                                to: approver.to_owned(),
                                from: env.contract.address.to_owned(),
                            },
                        ),
                        version: "2_0_0".to_string(),
                    }),
                    false => BankMsg::Send {
                        to_address: approver.to_string(),
                        amount: vec![Coin {
                            denom: ask_order.base.clone(),
                            amount: execute_size,
                        }],
                    }
                    .into(),
                },
                BankMsg::Send {
                    to_address: approver.to_string(),
                    amount: vec![Coin {
                        denom: bid_order.quote.clone(),
                        amount: quote_total,
                    }],
                }
                .into(),
                match matches!(
                    ProvenanceQuerier::new(&deps.querier)
                        .get_marker_by_denom(ask_order.base.clone()),
                    Ok(Marker {
                        marker_type: MarkerType::Restricted,
                        ..
                    })
                ) {
                    true => CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: provwasm_std::ProvenanceMsgParams::Marker(
                            MarkerMsgParams::TransferMarkerCoins {
                                coin: coin(execute_size.into(), converted_base.clone().denom),
                                to: bid_order.owner.to_owned(),
                                from: env.contract.address,
                            },
                        ),
                        version: "2_0_0".to_string(),
                    }),
                    false => BankMsg::Send {
                        to_address: bid_order.owner.to_owned().into(),
                        amount: vec![Coin {
                            denom: converted_base.clone().denom,
                            amount: execute_size,
                        }],
                    }
                    .into(),
                },
            ],
            attributes: vec![
                attr("action", "execute"),
                attr("ask_id", &ask_id),
                attr("bid_id", &bid_id),
                attr("base", &bid_order.base),
                attr("quote", &ask_order.quote),
                attr("price", &execute_price),
                attr("size", &execute_size),
            ],
            data: None,
        },
        AskOrderClass::Convertible { status } => {
            return Err(ContractError::AskOrderNotReady {
                current_status: format!("{:?}", status),
            });
        }
    };

    if !bidder_refund.is_zero() {
        response.messages.push(
            BankMsg::Send {
                    to_address: bid_order.owner.to_owned().into(),
                    amount:
                    // bid order completed, refund any remaining quote funds to bidder
                    vec![
                        Coin {
                            denom: bid_order.quote.clone(),
                            amount: bidder_refund,
                        },
                    ]
                }
            .into(),
        )
    }

    // finally update or remove the orders from storage
    if ask_order.size.is_zero() {
        get_ask_storage(deps.storage).remove(&ask_id.as_bytes());
    } else {
        get_ask_storage(deps.storage)
            .update(&ask_id.as_bytes(), |_| -> StdResult<_> { Ok(ask_order) })?;
    }

    if bid_order.size.is_zero() && bid_order.quote_size.is_zero() {
        get_bid_storage(deps.storage).remove(&bid_id.as_bytes());
    } else {
        get_bid_storage(deps.storage)
            .update(&bid_id.as_bytes(), |_| -> StdResult<_> { Ok(bid_order) })?;
    }

    Ok(response)
}

// smart contract migrate/upgrade entrypoint
#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    msg.validate()?;

    // migrate contract_info
    migrate_contract_info(deps.storage, deps.api, &msg)?;

    // migrate ask orders
    migrate_ask_orders(deps.storage, deps.api, &msg)?;

    // migrate bid orders
    migrate_bid_orders(deps.storage, deps.api, &msg)?;

    // lastly, migrate version_info
    migrate_version_info(deps.storage)?;

    Ok(Response::default())
}

// smart contract query entrypoint
#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    msg.validate()?;

    match msg {
        QueryMsg::GetAsk { id } => {
            let ask_storage_read = get_ask_storage_read(deps.storage);
            return to_binary(&ask_storage_read.load(id.as_bytes())?);
        }
        QueryMsg::GetBid { id } => {
            let bid_storage_read = get_bid_storage_read(deps.storage);
            return to_binary(&bid_storage_read.load(id.as_bytes())?);
        }
        QueryMsg::GetContractInfo {} => to_binary(&get_contract_info(deps.storage)?),
        QueryMsg::GetVersionInfo {} => to_binary(&get_version_info(deps.storage)?),
    }
}

// unit tests
#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::{coin, coins, Addr, BankMsg, Storage, Uint128};
    use cosmwasm_std::{from_binary, CosmosMsg};
    use provwasm_std::{
        Marker, MarkerMsgParams, NameMsgParams, ProvenanceMsg, ProvenanceMsgParams, ProvenanceRoute,
    };

    use crate::ask_order::{AskOrderClass, AskOrderV1};
    use crate::bid_order::get_bid_storage_read;

    use super::*;
    use provwasm_mocks::mock_dependencies;

    #[test]
    fn instantiate_valid_data() {
        // create valid init data
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("contract_admin", &[]);
        let init_msg = InstantiateMsg {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec!["approver_1".into(), "approver_2".into()],
            executors: vec!["exec_1".into(), "exec_2".into()],
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg.clone());

        // verify initialize response
        match init_response {
            Ok(init_response) => {
                assert_eq!(init_response.messages.len(), 1);
                assert_eq!(
                    init_response.messages[0],
                    CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Name,
                        params: ProvenanceMsgParams::Name(NameMsgParams::BindName {
                            name: init_msg.bind_name,
                            address: Addr::unchecked(MOCK_CONTRACT_ADDR),
                            restrict: true
                        }),
                        version: "2.0.0".to_string(),
                    })
                );
                let expected_contract_info = ContractInfoV1 {
                    name: "contract_name".into(),
                    bind_name: "contract_bind_name".into(),
                    base_denom: "base_denom".into(),
                    convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                    supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                    approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                    executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                    ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                    bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                    price_precision: Uint128(2),
                    size_increment: Uint128(100),
                };

                assert_eq!(init_response.attributes.len(), 2);
                assert_eq!(
                    init_response.attributes[0],
                    attr("contract_info", format!("{:?}", expected_contract_info))
                );
                assert_eq!(init_response.attributes[1], attr("action", "init"));
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
            bind_name: "".into(),
            base_denom: "".into(),
            convertible_base_denoms: vec![],
            supported_quote_denoms: vec![],
            approvers: vec![],
            executors: vec![],
            ask_required_attributes: vec![],
            bid_required_attributes: vec![],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg);

        // verify initialize response
        match init_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"name".into()));
                    assert!(fields.contains(&"bind_name".into()));
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
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec!["approver_1".into(), "approver_2".into()],
            executors: vec!["exec_1".into(), "exec_2".into()],
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(10),
        };

        // initialize
        let init_response = instantiate(deps.as_mut(), mock_env(), info, init_msg);

        // verify initialize response
        match init_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                InvalidPricePrecisionSizePair => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            base: "base_1".to_string(),
            size: Uint128(200),
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
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
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
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id,
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

    #[test]
    fn create_ask_convertible_base() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128(500),
        };

        let asker_info = mock_info("asker", &coins(500, "base_1"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );

        // verify create ask response
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
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
                assert_eq!(response.attributes[6], attr("price", "2"));
                assert_eq!(response.attributes[7], attr("size", "500"));
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
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id,
                            owner: asker_info.sender,
                            price,
                            quote,
                            size,
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

    #[test]
    fn create_ask_with_restricted_marker() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128(500),
        };

        let asker_info = mock_info("asker", &[]);

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );

        // verify create ask response
        match create_ask_response {
            Ok(response) => {
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
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
                assert_eq!(response.attributes[6], attr("price", "2"));
                assert_eq!(response.attributes[7], attr("size", "500"));
                assert_eq!(response.messages.len(), 1);

                match &response.messages[0] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: provwasm_std::ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(500, "base_1"),
                                        to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                                        from: Addr::unchecked("asker"),
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    message => panic!("expected marker transfer message, but was: {:?}", message),
                }
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
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrderV1 {
                            base,
                            class: AskOrderClass::Basic,
                            id,
                            owner: asker_info.sender,
                            price,
                            quote,
                            size,
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

    #[test]
    fn create_ask_with_restricted_marker_with_funds() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128(500),
        };

        let asker_info = mock_info("asker", &[coin(10, "base_1")]);

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                create_ask_response
            ),
        }
    }

    #[test]
    fn create_ask_existing_id() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2.5".into(),
            size: Uint128(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_1"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Ok(_) => {}
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // create ask data with existing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_2".into(),
            price: "4.5".into(),
            size: Uint128(400),
        };

        let asker_info = mock_info("asker", &coins(400, "base_1"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }

        // verify ask order stored is the original order
        let ask_storage = get_ask_storage_read(&deps.storage);

        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "base_1".into(),
                        class: AskOrderClass::Basic,
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2.5".into(),
                        quote: "quote_1".into(),
                        size: Uint128(200)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn create_ask_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask missing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "".into(),
            base: "".to_string(),
            quote: "".into(),
            price: "".into(),
            size: Uint128(0),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(0, "base_1")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                    assert!(fields.contains(&"base".into()));
                    assert!(fields.contains(&"quote".into()));
                    assert!(fields.contains(&"price".into()));
                    assert!(fields.contains(&"size".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_inconvertible_base() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask with inconvertible base
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "inconvertible".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128(100),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(100, "inconvertible")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InconvertibleBaseDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_unsupported_quote() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask with unsupported quote
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "unsupported".into(),
            price: "2".into(),
            size: Uint128(100),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(100, "base_denom")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedQuoteDenom => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_invalid_price_precision() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2.123".into(),
            size: Uint128(500),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(500, "base_denom")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
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
    fn create_ask_wrong_account_attributes() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128(200),
        };

        let asker_info = mock_info("asker", &coins(200, "base_denom"));

        // execute create ask
        let create_ask_response = execute(deps.as_mut(), mock_env(), asker_info, create_ask_msg);

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::Unauthorized => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            quote: "quote_1".into(),
            quote_size: Uint128(250),
            price: "2.5".into(),
            size: Uint128(100),
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
                assert_eq!(response.attributes.len(), 7);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(response.attributes[2], attr("base", "base_1"));
                assert_eq!(response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(response.attributes[4], attr("quote_size", "250"));
                assert_eq!(response.attributes[5], attr("price", "2.5"));
                assert_eq!(response.attributes[6], attr("size", "100"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
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
                        BidOrderV1 {
                            base,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote,
                            quote_size,
                            size,
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
    fn create_bid_with_restricted_marker() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            quote_size: Uint128(1000),
            price: "2".into(),
            size: Uint128(500),
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
                assert_eq!(response.attributes.len(), 7);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(
                    response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(response.attributes[2], attr("base", "base_1"));
                assert_eq!(response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(response.attributes[4], attr("quote_size", "1000"));
                assert_eq!(response.attributes[5], attr("price", "2"));
                assert_eq!(response.attributes[6], attr("size", "500"));
                assert_eq!(response.messages.len(), 1);

                match &response.messages[0] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: provwasm_std::ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(1000, "quote_1"),
                                        to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                                        from: Addr::unchecked("bidder"),
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    message => panic!("expected marker transfer message, but was: {:?}", message),
                }
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify bid order stored
        let ask_storage = get_bid_storage_read(&deps.storage);
        if let ExecuteMsg::CreateBid {
            id,
            base,
            quote,
            quote_size,
            price,
            size,
        } = create_bid_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV1 {
                            id,
                            owner: bidder_info.sender,
                            base,
                            quote,
                            quote_size,
                            price,
                            size,
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            quote_size: Uint128(10),
            price: "2".into(),
            size: Uint128(500),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
            quote: "quote_1".into(),
            quote_size: Uint128(250),
            price: "2.5".into(),
            size: Uint128(100),
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
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_1".into(),
            quote: "quote_1".into(),
            quote_size: Uint128(900),
            price: "4.5".into(),
            size: Uint128(200),
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
        let bid_storage = get_bid_storage_read(&deps.storage);

        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV1 {
                        base: "base_1".into(),
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2.5".into(),
                        quote: "quote_1".into(),
                        quote_size: Uint128(250),
                        size: Uint128(100),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid missing id
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "".into(),
            base: "".into(),
            quote: "".into(),
            quote_size: Uint128(0),
            price: "".into(),
            size: Uint128(0),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid with invalid base
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "notbasedenom".into(),
            quote: "quote_2".into(),
            quote_size: Uint128(200),
            price: "2".into(),
            size: Uint128(100),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_denom".into(),
            quote: "unsupported".into(),
            quote_size: Uint128(200),
            price: "2".into(),
            size: Uint128(100),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_denom".into(),
            quote: "quote_1".into(),
            quote_size: Uint128(100),
            price: "2".into(),
            size: Uint128(100),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_denom".into(),
            quote: "quote_1".into(),
            quote_size: Uint128(200),
            price: "2".into(),
            size: Uint128(100),
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
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            base: "base_denom".into(),
            quote: "quote_1".into(),
            quote_size: Uint128(200),
            price: "2.123".into(),
            size: Uint128(100),
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
    fn cancel_ask_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(100),
            },
        );

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(ask_storage.load("ask_id".as_bytes()).is_err(), true);
    }

    #[test]
    fn cancel_ask_convertible_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_denom"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };
        let cancel_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_ask_response {
            Ok(cancel_ask_response) => {
                assert_eq!(cancel_ask_response.attributes.len(), 2);
                assert_eq!(
                    cancel_ask_response.attributes[0],
                    attr("action", "cancel_ask")
                );
                assert_eq!(
                    cancel_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(cancel_ask_response.messages.len(), 2);
                assert_eq!(
                    cancel_ask_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender.to_string(),
                        amount: coins(100, "con_base_1"),
                    })
                );
                assert_eq!(
                    cancel_ask_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".to_string(),
                        amount: coins(100, "base_denom"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(ask_storage.load("ask_id".as_bytes()).is_err(), true);
    }

    #[test]
    fn cancel_ask_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let asker_info = mock_info("asker", &[]);

        // cancel ask order with missing id returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk { id: "".to_string() };
        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()))
                }
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_non_exist() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let asker_info = mock_info("asker", &[]);

        // cancel non-existent ask order returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::LoadOrderFailed { .. } => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_sender_notequal() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let asker_info = mock_info("asker", &[]);

        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Basic,
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("not_asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // cancel ask order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_ask_with_sent_funds() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // cancel ask order with sent_funds returns ContractError::CancelWithFunds
        let asker_info = mock_info("asker", &coins(1, "sent_coin"));
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), asker_info, cancel_ask_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::CancelWithFunds => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_bid_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // create bid data
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(200),
                price: "2".into(),
                size: Uint128(100),
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 2);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender.to_string(),
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(bid_storage.load("bid_id".as_bytes()).is_err(), true);
    }

    #[test]
    fn cancel_bid_restricted_marker() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(200),
                price: "2".into(),
                size: Uint128(100),
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_bid_response {
            Ok(cancel_bid_response) => {
                assert_eq!(cancel_bid_response.attributes.len(), 2);
                assert_eq!(
                    cancel_bid_response.attributes[0],
                    attr("action", "cancel_bid")
                );
                assert_eq!(
                    cancel_bid_response.attributes[1],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0],
                    CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: provwasm_std::ProvenanceMsgParams::Marker(
                            MarkerMsgParams::TransferMarkerCoins {
                                coin: coin(200, "quote_1"),
                                to: Addr::unchecked("bidder"),
                                from: Addr::unchecked(MOCK_CONTRACT_ADDR),
                            }
                        ),
                        version: "2_0_0".to_string()
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(bid_storage.load("bid_id".as_bytes()).is_err(), true);
    }

    #[test]
    fn cancel_bid_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let bidder_info = mock_info("bidder", &[]);

        // cancel bid order with missing id returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelAsk { id: "".to_string() };
        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()))
                }
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_bid_non_exist() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let bidder_info = mock_info("bidder", &[]);

        // cancel non-existent bid order returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::LoadOrderFailed { .. } => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_bid_sender_notequal() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let bidder_info = mock_info("bidder", &[]);

        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("not_bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(100),
                price: "2".into(),
                size: Uint128(200),
            },
        );

        // cancel bid order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn cancel_bid_with_sent_funds() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // cancel bid order with sent_funds returns ContractError::CancelWithFunds
        let bidder_info = mock_info("bidder", &coins(1, "sent_coin"));
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".to_string(),
        };

        let cancel_response = execute(deps.as_mut(), mock_env(), bidder_info, cancel_bid_msg);

        match cancel_response {
            Err(error) => match error {
                ContractError::CancelWithFunds => {}
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_valid_data() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(200),
                price: "2".into(),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_partial_ask_order() {
        // setup
        let mut deps =
            cosmwasm_std::testing::mock_dependencies(&[coin(30, "base_1"), coin(20, "quote_1")]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(30),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(20),
                price: "2".into(),
                size: Uint128(10),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(10),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "10"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(20, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(10, "base_1"),
                    })
                );
            }
        }

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "base_1".into(),
                        class: AskOrderClass::Basic,
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(20)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_partial_bid_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(50),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(200),
                price: "2".into(),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(50),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "50"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(100, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(50, "base_1"),
                    })
                );
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV1 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: "base_1".into(),
                        quote: "quote_1".into(),
                        quote_size: Uint128(100),
                        price: "2".into(),
                        size: Uint128(50),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(ask_storage.load("ask_id".as_bytes()).is_err(), true);
    }

    #[test]
    fn execute_partial_both_orders() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(600),
                price: "2".into(),
                size: Uint128(300),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "base_1".into(),
                        class: AskOrderClass::Basic,
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV1 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: "base_1".into(),
                        quote: "quote_1".into(),
                        quote_size: Uint128(400),
                        price: "2".into(),
                        size: Uint128(200),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    #[test]
    fn execute_convertible_partial_both_orders() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_2"),
                        converted_base: coin(200, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(600),
                price: "2".into(),
                size: Uint128(300),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "2"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(100, "con_base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(200, "quote_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order updated
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_2"),
                                converted_base: coin(100, "base_1"),
                            }
                        },

                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order update
        let bid_storage = get_bid_storage_read(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV1 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: "base_1".into(),
                        quote: "quote_1".into(),
                        quote_size: Uint128(400),
                        price: "2".into(),
                        size: Uint128(200),
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // remaining quote balance if remaining order size = 0
    #[test]
    fn execute_price_overlap_use_ask() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(500),
                price: "100.000000000000000000".into(),
                size: Uint128(5),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128(5),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(
                    execute_response.attributes[5],
                    attr("price", "2.000000000000000000")
                );
                assert_eq!(execute_response.attributes[6], attr("size", "5"));
                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            false
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_price_overlap_use_bid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: "base_1".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                price: "4".into(),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(400, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_convertible() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_denom"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_denom".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_denom"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(100, "con_base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[2],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_denom"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_restricted_marker_ask() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
                match &execute_response.messages[1] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(100, "base_1"),
                                        to: Addr::unchecked("bidder"),
                                        from: Addr::unchecked(MOCK_CONTRACT_ADDR)
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    _ => panic!(
                        "expected marker transfer, but received: {:?}",
                        execute_response.messages[1]
                    ),
                }
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_restricted_marker_bid() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let quote_marker_json = b"{
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

        let quote_marker: Marker = from_binary(&Binary::from(quote_marker_json)).unwrap();
        deps.querier.with_markers(vec![quote_marker]);

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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: ProvenanceMsgParams::Marker(MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(400, "quote_1"),
                            to: Addr::unchecked("asker"),
                            from: Addr::unchecked(MOCK_CONTRACT_ADDR)
                        }),
                        version: "2_0_0".to_string()
                    })
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(100, "base_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_restricted_marker_ask_and_bid() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let base_marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let quote_marker_json = b"{
              \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\",
              \"coins\": [
                {
                  \"denom\": \"quote_1\",
                  \"amount\": \"1000\"
                }
              ],
              \"account_number\": 11,
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
                  \"address\": \"tp1sfn6qfhpf9rw3ns8zrvate8qfya52tvgg5sc2w\"
                }
              ],
              \"status\": \"active\",
              \"denom\": \"quote_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let base_marker: Marker = from_binary(&Binary::from(base_marker_json)).unwrap();
        let quote_marker: Marker = from_binary(&Binary::from(quote_marker_json)).unwrap();
        deps.querier.with_markers(vec![base_marker, quote_marker]);

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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0],
                    CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: ProvenanceMsgParams::Marker(MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(400, "quote_1"),
                            to: Addr::unchecked("asker"),
                            from: Addr::unchecked(MOCK_CONTRACT_ADDR),
                        }),
                        version: "2_0_0".to_string(),
                    }),
                );
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Custom(ProvenanceMsg {
                        route: ProvenanceRoute::Marker,
                        params: ProvenanceMsgParams::Marker(MarkerMsgParams::TransferMarkerCoins {
                            coin: coin(100, "base_1"),
                            to: Addr::unchecked("bidder"),
                            from: Addr::unchecked(MOCK_CONTRACT_ADDR),
                        }),
                        version: "2_0_0".to_string(),
                    }),
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_convertible_restricted_marker() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

        let restricted_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let restricted_con_base_1 = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"con_base_1\",
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
              \"denom\": \"con_base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let marker_base_1: Marker = from_binary(&Binary::from(restricted_base_1)).unwrap();
        let marker_con_base_1: Marker = from_binary(&Binary::from(restricted_con_base_1)).unwrap();
        deps.querier
            .with_markers(vec![marker_base_1, marker_con_base_1]);

        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::Ready {
                        approver: Addr::unchecked("approver_1"),
                        converted_base: coin(100, "base_1"),
                    },
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(execute_response) => {
                assert_eq!(execute_response.attributes.len(), 7);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(
                    execute_response.attributes[1],
                    attr("ask_id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    execute_response.attributes[2],
                    attr("bid_id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(execute_response.attributes[3], attr("base", "base_1"));
                assert_eq!(execute_response.attributes[4], attr("quote", "quote_1"));
                assert_eq!(execute_response.attributes[5], attr("price", "4"));
                assert_eq!(execute_response.attributes[6], attr("size", "100"));
                assert_eq!(execute_response.messages.len(), 3);
                match &execute_response.messages[0] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(100, "con_base_1"),
                                        to: Addr::unchecked("approver_1"),
                                        from: Addr::unchecked(MOCK_CONTRACT_ADDR)
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    _ => panic!(
                        "expected marker transfer, but received: {:?}",
                        execute_response.messages[1]
                    ),
                }
                assert_eq!(
                    execute_response.messages[1],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
                match &execute_response.messages[2] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(100, "base_1"),
                                        to: Addr::unchecked("bidder"),
                                        from: Addr::unchecked(MOCK_CONTRACT_ADDR)
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    _ => panic!(
                        "expected marker transfer, but received: {:?}",
                        execute_response.messages[1]
                    ),
                }
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_err(),
            true
        );
    }

    #[test]
    fn execute_invalid_data() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "".into(),
            bid_id: "".into(),
            price: "0".into(),
            size: Uint128::zero(),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"ask_id".into()));
                    assert!(fields.contains(&"bid_id".into()));
                }
                _ => {
                    panic!("unexpected error: {:?}", error)
                }
            },
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_by_non_executor() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // execute by non-executor
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(1),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::Unauthorized) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_ask_not_ready() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );
        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(200),
            },
        );

        // execute when ask order not ready returns ContractError::PendingIssuerApproval
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::AskOrderNotReady { current_status }) => {
                assert_eq!(
                    current_status,
                    format!("{:?}", AskOrderStatus::PendingIssuerApproval)
                )
            }
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_ask_non_exist() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(100),
                size: Uint128(200),
            },
        );

        // execute on non-existent ask order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::LoadOrderFailed { .. }) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_bid_non_exist() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(200),
            },
        );

        // execute on non-existent bid order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::LoadOrderFailed { .. }) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_with_sent_funds() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                supported_quote_denoms: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // execute with sent_funds returns ContractError::ExecuteWithFunds
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(1),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &coins(100, "funds")),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::ExecuteWithFunds) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_price_mismatch() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                price: "3".into(),
                quote: "quote_1".into(),
                size: Uint128(300),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(100),
                size: Uint128(200),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(ContractError::AskBidPriceMismatch) => {}
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }
    }

    #[test]
    fn execute_price_not_ask_or_bid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "6".into(),
            size: Uint128(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(ContractError::InvalidExecutePrice) => (),
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_ok(),
            true
        );

        // verify bid order still exists
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_ok(),
            true
        );
    }

    #[test]
    fn execute_size_greater_than_ask_and_bid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
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
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV1 {
                base: "base_1".into(),
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: "quote_1".into(),
                quote_size: Uint128(400),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128(200),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Err(ContractError::InvalidExecuteSize) => (),
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_ok(),
            true
        );

        // verify bid order still exists
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage
                .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
                .is_ok(),
            true
        );
    }

    #[test]
    fn approve_ask_valid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(approve_ask_response) => {
                assert_eq!(approve_ask_response.attributes.len(), 6);
                assert_eq!(
                    approve_ask_response.attributes[0],
                    attr("action", "approve_ask")
                );
                assert_eq!(
                    approve_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    approve_ask_response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_denom")
                            },
                        })
                        .unwrap()
                    )
                );
                assert_eq!(approve_ask_response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(approve_ask_response.attributes[4], attr("price", "2"));
                assert_eq!(approve_ask_response.attributes[5], attr("size", "100"));
                assert_eq!(approve_ask_response.messages.len(), 0);
            }
        }

        // verify ask order update
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_denom"),
                            },
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_restricted_marker() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => panic!("unexpected error: {:?}", error),
            Ok(approve_ask_response) => {
                assert_eq!(approve_ask_response.attributes.len(), 6);
                assert_eq!(
                    approve_ask_response.attributes[0],
                    attr("action", "approve_ask")
                );
                assert_eq!(
                    approve_ask_response.attributes[1],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    approve_ask_response.attributes[2],
                    attr(
                        "class",
                        serde_json::to_string(&AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_1")
                            },
                        })
                        .unwrap()
                    )
                );
                assert_eq!(approve_ask_response.attributes[3], attr("quote", "quote_1"));
                assert_eq!(approve_ask_response.attributes[4], attr("price", "2"));
                assert_eq!(approve_ask_response.attributes[5], attr("size", "100"));
                assert_eq!(approve_ask_response.messages.len(), 1);
                match &approve_ask_response.messages[0] {
                    CosmosMsg::Custom(message) => {
                        assert_eq!(
                            message,
                            &ProvenanceMsg {
                                route: ProvenanceRoute::Marker,
                                params: provwasm_std::ProvenanceMsgParams::Marker(
                                    MarkerMsgParams::TransferMarkerCoins {
                                        coin: coin(100, "base_1"),
                                        to: Addr::unchecked(MOCK_CONTRACT_ADDR),
                                        from: Addr::unchecked("approver_1"),
                                    }
                                ),
                                version: "2_0_0".to_string()
                            }
                        )
                    }
                    _ => panic!(
                        "expected marker transfer message, but received: {:?}",
                        approve_ask_response.messages[0]
                    ),
                }
            }
        }

        // verify ask order update
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::Ready {
                                approver: Addr::unchecked("approver_1"),
                                converted_base: coin(100, "base_1"),
                            },
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_id() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "59e82f8f-268e-433f-9711-e9f2d2cc19a5".into(),
                base: "base_denom".to_string(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_converted_base_denom() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "wrong_base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "wrong_base_denom".to_string(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_wrong_converted_base_amount() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(101, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_converted_base_amount_sent_funds_mismatch() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128(99),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::SentFundsOrderMismatch => {}
                error => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_restricted_marker_with_funds() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(10, "gme")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                approve_ask_response
            ),
        }
    }

    #[test]
    fn approve_ask_restricted_marker_order_size_mismatch() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        let marker_json = b"{
              \"address\": \"tp18vmzryrvwaeykmdtu6cfrz5sau3dhc5c73ms0u\",
              \"coins\": [
                {
                  \"denom\": \"base_1\",
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
              \"denom\": \"base_1\",
              \"total_supply\": \"1000\",
              \"marker_type\": \"restricted\",
              \"supply_fixed\": false
            }";

        let test_marker: Marker = from_binary(&Binary::from(marker_json)).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128(101),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(ContractError::SentFundsOrderMismatch) => (),
            _ => panic!(
                "expected ContractError::SentFundsOrderMismatch, but received: {:?}",
                approve_ask_response
            ),
        }

        // verify ask order update
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_not_approver() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrderV1 {
                base: "con_base_1".into(),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("not_approver", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::Unauthorized => {}
                _ => panic!("unexpected error: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order unchanged
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrderV1 {
                        base: "con_base_1".into(),
                        class: AskOrderClass::Convertible {
                            status: AskOrderStatus::PendingIssuerApproval,
                        },
                        id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                        owner: Addr::unchecked("asker"),
                        price: "2".into(),
                        quote: "quote_1".into(),
                        size: Uint128(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn query_contract_info() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // query for contract_info
        let query_contract_info_response =
            query(deps.as_ref(), mock_env(), QueryMsg::GetContractInfo {});

        match query_contract_info_response {
            Ok(contract_info) => {
                assert_eq!(
                    contract_info,
                    to_binary(&get_contract_info(&deps.storage).unwrap()).unwrap()
                )
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }

    #[test]
    fn query_ask_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid ask order
        let ask_order = AskOrderV1 {
            base: "base_1".into(),
            class: AskOrderClass::Basic,
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128(200),
        };

        let mut ask_storage = get_ask_storage(&mut deps.storage);
        if let Err(error) = ask_storage.save(
            &"ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes(),
            &ask_order,
        ) {
            panic!("unexpected error: {:?}", error)
        };

        // verify ask order still exists
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage
                .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
                .is_ok(),
            true
        );

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            },
        );

        assert_eq!(query_ask_response, to_binary(&ask_order));
    }

    #[test]
    fn query_bid_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV1 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        );

        // store valid bid order
        let bid_order = BidOrderV1 {
            base: "base_1".into(),
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            owner: Addr::unchecked("bidder"),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128(100),
            size: Uint128(100),
        };

        let mut bid_storage = get_bid_storage(&mut deps.storage);
        if let Err(error) = bid_storage.save(&bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };

        // query for bid order
        let query_bid_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBid {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            },
        );

        assert_eq!(query_bid_response, to_binary(&bid_order));
    }

    fn setup_test_base(storage: &mut dyn Storage, contract_info: &ContractInfoV1) {
        if let Err(error) = set_contract_info(storage, contract_info) {
            panic!("unexpected error: {:?}", error)
        }
    }

    fn store_test_ask(storage: &mut dyn Storage, ask_order: &AskOrderV1) {
        let mut ask_storage = get_ask_storage(storage);
        if let Err(error) = ask_storage.save(&ask_order.id.as_bytes(), &ask_order) {
            panic!("unexpected error: {:?}", error)
        };
    }

    fn store_test_bid(storage: &mut dyn Storage, bid_order: &BidOrderV1) {
        let mut bid_storage = get_bid_storage(storage);
        if let Err(error) = bid_storage.save(&bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };
    }
}
