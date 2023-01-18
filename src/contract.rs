use cosmwasm_std::{
    attr, coin, coins, entry_point, to_binary, Addr, BankMsg, Binary, Coin, Deps, DepsMut, Env,
    MessageInfo, Order, Record, Response, StdError, StdResult, Uint128,
};
use provwasm_std::{
    bind_name, transfer_marker_coins, NameBinding, ProvenanceMsg, ProvenanceQuerier,
    ProvenanceQuery,
};

use crate::ask_order::{
    get_ask_storage, get_ask_storage_read, migrate_ask_orders, AskOrderClass, AskOrderStatus,
    AskOrderV1,
};
use crate::bid_order::{get_bid_storage, get_bid_storage_read, migrate_bid_orders, BidOrderV2};
use crate::common::{Action, Event, FeeInfo};
use crate::contract_info::{
    get_contract_info, migrate_contract_info, modify_contract_info, set_contract_info,
    ContractInfoV3,
};
use crate::error::ContractError;
use crate::error::ContractError::InvalidPricePrecisionSizePair;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Validate};
use crate::util::{is_invalid_price_precision, is_restricted_marker};
use crate::version_info::{
    get_version_info, migrate_version_info, set_version_info, VersionInfoV1, CRATE_NAME,
    PACKAGE_VERSION,
};
use rust_decimal::prelude::{FromPrimitive, FromStr, ToPrimitive, Zero};
use rust_decimal::{Decimal, RoundingStrategy};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::Mul;

// smart contract initialization entrypoint
#[entry_point]
pub fn instantiate(
    deps: DepsMut<ProvenanceQuery>,
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

    // validate and set ask fee
    let ask_fee = match (&msg.ask_fee_account, &msg.ask_fee_rate) {
        (Some(account), Some(rate)) => match (account.as_str(), rate.as_str()) {
            ("", "") => None,
            (_, _) => {
                Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                    fields: vec![String::from("ask_fee_rate")],
                })?;

                Some(FeeInfo {
                    account: deps.api.addr_validate(account)?,
                    rate: rate.to_string(),
                })
            }
        },
        (_, _) => None,
    };

    // validate and set bid fee
    let bid_fee = match (&msg.bid_fee_account, &msg.bid_fee_rate) {
        (Some(account), Some(rate)) => match (account.as_str(), rate.as_str()) {
            ("", "") => None,
            (_, _) => {
                Decimal::from_str(rate).map_err(|_| ContractError::InvalidFields {
                    fields: vec![String::from("bid_fee_rate")],
                })?;

                Some(FeeInfo {
                    account: deps.api.addr_validate(account)?,
                    rate: rate.to_string(),
                })
            }
        },
        (_, _) => None,
    };

    // set contract info
    let contract_info = ContractInfoV3 {
        name: msg.name,
        bind_name: msg.bind_name,
        base_denom: msg.base_denom,
        convertible_base_denoms: msg.convertible_base_denoms,
        supported_quote_denoms: msg.supported_quote_denoms,
        approvers,
        executors,
        ask_fee_info: ask_fee,
        bid_fee_info: bid_fee,
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

    set_version_info(
        deps.storage,
        &VersionInfoV1 {
            version: PACKAGE_VERSION.to_string(),
            definition: CRATE_NAME.to_string(),
        },
    )?;

    // build response
    Ok(Response::new()
        .add_message(bind_name_msg)
        .add_attributes(vec![
            attr(
                "contract_info",
                format!("{:?}", get_contract_info(deps.storage)?),
            ),
            attr("action", "init"),
        ]))
}

// smart contract execute entrypoint
#[entry_point]
pub fn execute(
    deps: DepsMut<ProvenanceQuery>,
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
            fee,
            price,
            quote,
            quote_size,
            size,
        } => create_bid(
            deps,
            env,
            &info,
            BidOrderV2 {
                base: Coin {
                    amount: size,
                    denom: base,
                },
                events: vec![],
                fee,
                id,
                owner: info.sender.to_owned(),
                price,
                quote: Coin {
                    amount: quote_size,
                    denom: quote,
                },
            },
        ),
        ExecuteMsg::CancelAsk { id } => cancel_ask(deps, env, info, id),
        ExecuteMsg::CancelBid { id } => {
            reverse_bid(deps, env, info, id, String::from("cancel_bid"), None)
        }
        ExecuteMsg::ExecuteMatch {
            ask_id,
            bid_id,
            price,
            size,
        } => execute_match(deps, env, info, ask_id, bid_id, price, size),
        ExecuteMsg::ExpireAsk { id } => {
            reverse_ask(deps, env, info, id, String::from("expire_ask"), None)
        }
        ExecuteMsg::ExpireBid { id } => {
            reverse_bid(deps, env, info, id, String::from("expire_bid"), None)
        }
        ExecuteMsg::RejectAsk { id, size } => {
            reverse_ask(deps, env, info, id, String::from("reject_ask"), size)
        }
        ExecuteMsg::RejectBid { id, size } => {
            reverse_bid(deps, env, info, id, String::from("reject_bid"), size)
        }
        ExecuteMsg::ModifyContract {
            approvers,
            executors,
            ask_fee_rate,
            ask_fee_account,
            bid_fee_rate,
            bid_fee_account,
            ask_required_attributes,
            bid_required_attributes,
        } => modify_contract(
            deps,
            env,
            &info,
            approvers,
            executors,
            ask_fee_rate,
            ask_fee_account,
            bid_fee_rate,
            bid_fee_account,
            ask_required_attributes,
            bid_required_attributes,
        ),
    }
}

fn approve_ask(
    deps: DepsMut<ProvenanceQuery>,
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

    let is_base_restricted_marker = is_restricted_marker(&deps.querier, base.clone());

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
                    // Validate ask order hasnt been approved yet
                    match stored_ask_order.class {
                        AskOrderClass::Convertible { status } => match status {
                            AskOrderStatus::Ready {
                                approver,
                                converted_base: _converted_base,
                            } => {
                                return Err(ContractError::AskOrderReady {
                                    approver: approver.to_string(),
                                })
                            }
                            _ => {}
                        },
                        _ => {}
                    }

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
    let mut response = Response::new().add_attributes(vec![
        attr("action", "approve_ask"),
        attr("id", &updated_ask_order.id),
        attr("class", serde_json::to_string(&updated_ask_order.class)?),
        attr("quote", &updated_ask_order.quote),
        attr("price", &updated_ask_order.price),
        attr("size", &updated_ask_order.size.to_string()),
    ]);

    if is_base_restricted_marker {
        response = response.add_message(transfer_marker_coins(
            size.into(),
            base,
            env.contract.address,
            info.sender,
        )?);
    }

    Ok(response)
}

// create ask entrypoint
fn create_ask(
    deps: DepsMut<ProvenanceQuery>,
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
    let is_base_restricted_marker = is_restricted_marker(&deps.querier, ask_order.base.clone());

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
    if is_invalid_price_precision(ask_price.clone(), contract_info.price_precision.clone()) {
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

    ask_storage.save(ask_order.id.as_bytes(), &ask_order)?;

    let mut response = Response::new().add_attributes(vec![
        attr("action", "create_ask"),
        attr("id", &ask_order.id),
        attr("class", serde_json::to_string(&ask_order.class)?),
        attr("target_base", &contract_info.base_denom),
        attr("base", &ask_order.base),
        attr("quote", &ask_order.quote),
        attr("price", &ask_order.price),
        attr("size", &ask_order.size.to_string()),
    ]);

    if is_base_restricted_marker {
        response = response.add_message(transfer_marker_coins(
            ask_order.size.into(),
            ask_order.base.to_owned(),
            env.contract.address,
            ask_order.owner,
        )?);
    }

    Ok(response)
}

// create bid entrypoint
fn create_bid(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: &MessageInfo,
    mut bid_order: BidOrderV2,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    let bid_price =
        Decimal::from_str(&bid_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("price")],
        })?;

    // error if price smaller than allow price precision
    if is_invalid_price_precision(bid_price.clone(), contract_info.price_precision.clone()) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

    // error if order size is not multiple of size_increment
    if (bid_order.base.amount.u128() % contract_info.size_increment.u128()).ne(&0) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    // calculate quote total (price * size), error if overflows
    let total = bid_price
        .checked_mul(Decimal::from(bid_order.base.amount.u128()))
        .ok_or(ContractError::TotalOverflow)?;

    // error if total is not an integer
    if total.fract().ne(&Decimal::zero()) {
        return Err(ContractError::NonIntegerTotal);
    }

    // Validate the quote.amount matches the base.amount * price
    if total.ne(&Decimal::from(bid_order.quote.amount.u128())) {
        return Err(ContractError::SentFundsOrderMismatch);
    }

    // if bid fee exists, calculate and compare to sent fee_size
    match contract_info.bid_fee_info {
        Some(bid_fee_info) => {
            let bid_fee_rate = Decimal::from_str(&bid_fee_info.rate).map_err(|_| {
                ContractError::InvalidFields {
                    fields: vec![String::from("ContractInfo.bid_fee_info.rate")],
                }
            })?;

            // if the bid fee rate is 0, there should not be any sent fees
            if bid_fee_rate.eq(&Decimal::zero()) && bid_order.fee.is_some() {
                return Err(ContractError::SentFees);
            }

            let calculated_fee_size = bid_fee_rate
                .checked_mul(total)
                .ok_or(ContractError::TotalOverflow)?
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_u128()
                .ok_or(ContractError::TotalOverflow)?;

            match &mut bid_order.fee {
                Some(fee) => {
                    if fee.amount.ne(&Uint128::new(calculated_fee_size)) {
                        return Err(ContractError::InvalidFeeSize {
                            fee_rate: bid_fee_info.rate,
                        });
                    }
                    if fee.denom.ne(&bid_order.quote.denom) {
                        return Err(ContractError::SentFundsOrderMismatch);
                    }
                }
                _ => {
                    return Err(ContractError::InvalidFeeSize {
                        fee_rate: bid_fee_info.rate,
                    })
                }
            }
        }
        None => {
            if bid_order.fee.is_some() {
                return Err(ContractError::SentFees);
            }
        }
    }

    // error if order quote is not supported quote denom
    if !&contract_info
        .supported_quote_denoms
        .contains(&bid_order.quote.denom)
    {
        return Err(ContractError::UnsupportedQuoteDenom);
    }

    // error if order base denom not equal to contract base denom
    if bid_order.base.denom.ne(&contract_info.base_denom) {
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

    // is bid quote a marker
    let is_quote_restricted_marker =
        is_restricted_marker(&deps.querier, bid_order.quote.denom.clone());

    // determine sent funds requirements
    if is_quote_restricted_marker && !info.funds.is_empty() {
        // no funds should be sent if base is a restricted marker
        return Err(ContractError::SentFundsOrderMismatch);
    }

    // sent funds must match order if not a restricted marker
    if !is_quote_restricted_marker
        && info.funds.ne(&coins(
            match &bid_order.fee {
                Some(fee) => total.to_u128().unwrap() + fee.amount.u128(),
                _ => total.to_u128().unwrap(),
            },
            bid_order.quote.denom.to_owned(),
        ))
    {
        return Err(ContractError::SentFundsOrderMismatch);
    }

    let mut bid_storage = get_bid_storage(deps.storage);

    if bid_storage.may_load(bid_order.id.as_bytes())?.is_some() {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("id")],
        });
    }

    bid_storage.save(bid_order.id.as_bytes(), &bid_order)?;

    let mut response = Response::new().add_attributes(vec![
        attr("action", "create_bid"),
        attr("base", &bid_order.base.denom),
        attr("id", &bid_order.id),
        attr(
            "fee",
            match &bid_order.fee {
                Some(fee) => format!("{:?}", fee),
                _ => "None".into(),
            },
        ),
        attr("price", &bid_order.price),
        attr("quote", &bid_order.quote.denom),
        attr("quote_size", &bid_order.quote.amount.to_string()),
        attr("size", &bid_order.base.amount.to_string()),
    ]);

    if is_quote_restricted_marker {
        response = response.add_message(transfer_marker_coins(
            match bid_order.fee {
                Some(fees) => (bid_order.quote.amount + fees.amount).into(),
                _ => bid_order.quote.amount.into(),
            },
            bid_order.quote.denom.to_owned(),
            env.contract.address,
            bid_order.owner,
        )?);
    }

    Ok(response)
}

// cancel ask entrypoint
fn cancel_ask(
    deps: DepsMut<ProvenanceQuery>,
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

    // is ask base a marker
    let is_base_restricted_marker = is_restricted_marker(&deps.querier, base.clone());

    // return 'base' to owner, return converted_base to issuer if applicable
    let mut response = Response::new()
        .add_message(match is_base_restricted_marker {
            true => {
                transfer_marker_coins(size.into(), base, owner, env.contract.address.to_owned())?
            }
            false => BankMsg::Send {
                to_address: owner.to_string(),
                amount: coins(u128::from(size), base),
            }
            .into(),
        })
        .add_attributes(vec![attr("action", "cancel_ask"), attr("id", id)]);

    if let AskOrderClass::Convertible {
        status: AskOrderStatus::Ready {
            approver,
            converted_base,
        },
    } = class
    {
        // is convertible a marker
        let is_convertible_restricted_marker =
            is_restricted_marker(&deps.querier, converted_base.denom.clone());

        response = response.add_message(match is_convertible_restricted_marker {
            true => transfer_marker_coins(
                converted_base.amount.into(),
                converted_base.denom,
                approver,
                env.contract.address,
            )?,
            false => BankMsg::Send {
                to_address: approver.to_string(),
                amount: vec![converted_base],
            }
            .into(),
        });
    }

    Ok(response)
}

// reverse ask entrypoint
fn reverse_ask(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    id: String,
    action: String,
    cancel_size: Option<Uint128>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // return error if id is empty
    if id.is_empty() {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::ExpireWithFunds);
    }

    let contract_info = get_contract_info(deps.storage)?;

    if !contract_info.executors.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    let ask_storage = get_ask_storage_read(deps.storage);

    // retrieve the order
    let mut ask_order = ask_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    // determine the effective cancel size
    let effective_cancel_size = match cancel_size {
        None => ask_order.size,
        Some(cancel_size) => cancel_size,
    };

    // error if cancel size is not multiple of size_increment
    if (effective_cancel_size.u128() % contract_info.size_increment.u128()).ne(&0) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    // subtract the cancel size from the order size
    ask_order.size = ask_order
        .size
        .checked_sub(effective_cancel_size)
        .map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("size")],
        })?;

    // is ask base a marker
    let is_quote_restricted_marker = is_restricted_marker(&deps.querier, ask_order.base.clone());

    // return 'base' to owner, return converted_base to issuer if applicable
    let mut response = Response::new()
        .add_message(match is_quote_restricted_marker {
            true => transfer_marker_coins(
                effective_cancel_size.into(),
                ask_order.base.to_owned(),
                ask_order.owner.to_owned(),
                env.contract.address.to_owned(),
            )?,
            false => BankMsg::Send {
                to_address: ask_order.owner.to_string(),
                amount: coins(u128::from(effective_cancel_size), ask_order.base.to_owned()),
            }
            .into(),
        })
        .add_attributes(vec![
            attr("action", action),
            attr("id", id),
            attr("reverse_size", effective_cancel_size),
        ]);

    if let AskOrderClass::Convertible {
        status: AskOrderStatus::Ready {
            approver,
            converted_base,
        },
    } = ask_order.class.to_owned()
    {
        // is convertible a marker
        let is_convertible_restricted_marker =
            is_restricted_marker(&deps.querier, converted_base.denom.clone());

        response = response.add_message(match is_convertible_restricted_marker {
            true => transfer_marker_coins(
                effective_cancel_size.into(),
                converted_base.denom,
                approver,
                env.contract.address,
            )?,
            false => BankMsg::Send {
                to_address: approver.to_string(),
                amount: coins(u128::from(effective_cancel_size), converted_base.denom),
            }
            .into(),
        });
    }

    let mut ask_storage = get_ask_storage(deps.storage);

    // remove the ask order from storage if remaining size is 0, otherwise, store updated order
    if ask_order.size.is_zero() {
        ask_storage.remove(ask_order.id.as_bytes());
        response = response.add_attributes(vec![attr("order_open", "false")]);
    } else {
        ask_storage.save(ask_order.id.as_bytes(), &ask_order)?;
        response = response.add_attributes(vec![attr("order_open", "true")]);
    }

    Ok(response)
}

// reverse bid entrypoint
fn reverse_bid(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    id: String,
    action: String,
    cancel_size: Option<Uint128>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // return error if id is empty
    if id.is_empty() {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::ExpireWithFunds);
    }

    let contract_info = get_contract_info(deps.storage)?;

    let bid_storage = get_bid_storage_read(deps.storage);

    //load the bid order
    let mut bid_order = bid_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    if action == "cancel_bid" {
        if !info.sender.eq(&bid_order.owner) {
            return Err(ContractError::Unauthorized);
        }
    } else if !contract_info.executors.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    // determine the effective cancel size
    let effective_cancel_size = match cancel_size {
        None => bid_order.get_remaining_base(),
        Some(cancel_size) => cancel_size,
    };

    // error if cancel size is not multiple of size_increment
    if (effective_cancel_size.u128() % contract_info.size_increment.u128()).ne(&0) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    // error if cancel size is greater than available base size
    if bid_order.get_remaining_base().lt(&effective_cancel_size) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("size")],
        });
    }

    // calculate canceled quote size (price * effective_cancel_size), error if overflows
    let effective_cancel_quote_size = Decimal::from_str(&bid_order.price)
        .unwrap()
        .mul(Decimal::from(effective_cancel_size.u128()));

    // error if canceled quote total is not an integer
    if effective_cancel_quote_size.fract().ne(&Decimal::zero()) {
        return Err(ContractError::NonIntegerTotal);
    }

    let effective_cancel_quote_size = Uint128::new(effective_cancel_quote_size.to_u128().unwrap());

    // calculate canceled fee size
    let effective_cancel_fee_size = match &bid_order.fee {
        Some(bid_fee) => {
            let quote_remaining_ratio = bid_order
                .get_quote_ratio(bid_order.get_remaining_quote() - effective_cancel_quote_size);

            // fees required for remaining quote
            let required_remaining_fees = Decimal::from_u128(bid_fee.amount.u128())
                .unwrap()
                .mul(quote_remaining_ratio)
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_u128()
                .unwrap();

            // available fees - fees required = canceled/returned fees
            let effective_cancel_fee_size = bid_order
                .get_remaining_fee()
                .checked_sub(required_remaining_fees.into())
                .map_err(|_| ContractError::InvalidFields {
                    fields: vec![String::from("size")],
                })?;

            Some(Coin {
                amount: effective_cancel_fee_size,
                denom: bid_order.quote.denom.to_owned(),
            })
        }
        _ => None,
    };

    // is bid quote a marker
    let is_quote_restricted_marker =
        is_restricted_marker(&deps.querier, bid_order.quote.denom.clone());

    // add event to order
    bid_order.events.push(Event {
        action: Action::Reject {
            base: Coin {
                amount: effective_cancel_size,
                denom: bid_order.base.denom.to_owned(),
            },
            fee: effective_cancel_fee_size.to_owned(),
            quote: Coin {
                amount: effective_cancel_quote_size,
                denom: bid_order.quote.denom.to_owned(),
            },
        },
        block_info: env.block.into(),
    });

    // 'send quote back to owner' message
    let mut response = Response::new()
        .add_message(match is_quote_restricted_marker {
            true => transfer_marker_coins(
                effective_cancel_quote_size.u128(),
                bid_order.quote.denom.to_owned(),
                bid_order.owner.to_owned(),
                env.contract.address.to_owned(),
            )?,
            false => BankMsg::Send {
                to_address: bid_order.owner.to_string(),
                amount: vec![coin(
                    effective_cancel_quote_size.u128(),
                    bid_order.quote.denom.to_owned(),
                )],
            }
            .into(),
        })
        .add_attributes(vec![
            attr("action", action),
            attr("id", id),
            attr("reverse_size", effective_cancel_size),
        ]);

    // add 'send fee back to owner' message
    if let Some(fee) = effective_cancel_fee_size {
        response = response.add_message(match is_quote_restricted_marker {
            true => transfer_marker_coins(
                fee.amount.u128(),
                bid_order.quote.denom.to_owned(),
                bid_order.owner.to_owned(),
                env.contract.address,
            )?,
            false => BankMsg::Send {
                to_address: bid_order.owner.to_string(),
                amount: vec![coin(fee.amount.u128(), bid_order.quote.denom.to_owned())],
            }
            .into(),
        });
    }

    let mut bid_storage = get_bid_storage(deps.storage);

    // remove the bid order from storage if remaining size is 0, otherwise, store updated order
    match bid_order.get_remaining_base().is_zero() {
        true => {
            bid_storage.remove(bid_order.id.as_bytes());
            response = response.add_attributes(vec![attr("order_open", "false")]);
        }
        false => {
            bid_storage.save(bid_order.id.as_bytes(), &bid_order)?;
            response = response.add_attributes(vec![attr("order_open", "true")]);
        }
    }

    Ok(response)
}

fn modify_contract(
    deps: DepsMut<ProvenanceQuery>,
    _env: Env,
    info: &MessageInfo,
    approvers: Option<Vec<String>>,
    executors: Option<Vec<String>>,
    ask_fee_rate: Option<String>,
    ask_fee_account: Option<String>,
    bid_fee_rate: Option<String>,
    bid_fee_account: Option<String>,
    ask_required_attributes: Option<Vec<String>>,
    bid_required_attributes: Option<Vec<String>>,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    if !contract_info.executors.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    let ask_storage = get_ask_storage_read(deps.storage);
    let ask_orders: Vec<AskOrderV1> = ask_storage
        .range(None, None, Order::Ascending)
        .map(|kv_ask: StdResult<Record<AskOrderV1>>| {
            let (_, ask_order) = kv_ask.unwrap();
            ask_order
        })
        .collect();
    if !ask_orders.is_empty() {
        match &ask_required_attributes {
            None => (),
            Some(_) => {
                return Err(ContractError::InvalidFields {
                    fields: vec!["ask_required_attributes".to_string()],
                });
            }
        }
    }

    let bid_storage = get_bid_storage_read(deps.storage);
    let bid_orders: Vec<BidOrderV2> = bid_storage
        .range(None, None, Order::Ascending)
        .map(|kv_bid: StdResult<Record<BidOrderV2>>| {
            let (_, bid_order) = kv_bid.unwrap();
            bid_order
        })
        .collect();
    if !bid_orders.is_empty() {
        match &bid_required_attributes {
            None => {}
            Some(_) => {
                return Err(ContractError::InvalidFields {
                    fields: vec!["bid_required_attributes".to_string()],
                });
            }
        }
        match (&bid_fee_rate, &bid_fee_account) {
            (None, None) => {}
            (_, _) => {
                return Err(ContractError::InvalidFields {
                    fields: vec!["bid_fee".to_string()],
                });
            }
        }
    }

    if !ask_orders.is_empty() || !bid_orders.is_empty() {
        match &approvers {
            None => {}
            Some(approvers) => {
                let current_approvers: HashSet<String> = contract_info
                    .approvers
                    .into_iter()
                    .map(|item| item.into_string())
                    .collect();
                let new_approvers: HashSet<String> = approvers.clone().into_iter().collect();
                if !current_approvers.is_subset(&new_approvers) {
                    return Err(ContractError::InvalidFields {
                        fields: vec!["approvers".to_string()],
                    });
                }
            }
        }
    }

    modify_contract_info(
        deps,
        approvers,
        executors,
        ask_fee_rate,
        ask_fee_account,
        bid_fee_rate,
        bid_fee_account,
        ask_required_attributes,
        bid_required_attributes,
    )?;

    let response = Response::new();

    Ok(response)
}

// match and execute an ask and bid order
fn execute_match(
    deps: DepsMut<ProvenanceQuery>,
    env: Env,
    info: MessageInfo,
    ask_id: String,
    bid_id: String,
    price: String,
    execute_size: Uint128,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    // only executors may execute matches
    if !contract_info.executors.contains(&info.sender) {
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

    // Validate the requested quote denom in the ask order matches the offered quote denom in the bid order
    if ask_order.quote.ne(&bid_order.quote.denom) {
        return Err(ContractError::UnsupportedQuoteDenom);
    }

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
    if execute_size.gt(&ask_order.size) || execute_size.gt(&bid_order.get_remaining_base()) {
        return Err(ContractError::InvalidExecuteSize);
    }

    // calculate gross proceeds using execute price, (price * size), error if overflows
    let actual_gross_proceeds = execute_price
        .checked_mul(Decimal::from(execute_size.u128()))
        .ok_or(ContractError::TotalOverflow)?;

    // error if gross proceeds is not an integer
    if actual_gross_proceeds.fract().ne(&Decimal::zero()) {
        return Err(ContractError::NonIntegerTotal);
    }

    let mut net_proceeds = Uint128::new(
        actual_gross_proceeds
            .to_u128()
            .ok_or(ContractError::TotalOverflow)?,
    );

    ask_order.size -= execute_size;

    if let AskOrderClass::Convertible {
        status: AskOrderStatus::Ready { converted_base, .. },
    } = &mut ask_order.class
    {
        converted_base.amount = ask_order.size;
    }

    // is base a restricted marker
    let is_base_restricted_marker = is_restricted_marker(&deps.querier, ask_order.base.clone());

    // is quote a restricted marker
    let is_quote_restricted_marker =
        is_restricted_marker(&deps.querier, bid_order.quote.denom.clone());

    let mut response = Response::new();
    response = response.add_attributes(vec![
        attr("action", "execute"),
        attr("ask_id", &ask_id),
        attr("bid_id", &bid_id),
        attr("base", &bid_order.base.denom),
        attr("quote", &ask_order.quote),
        attr("price", &execute_price.to_string()),
        attr("size", &execute_size.to_string()),
    ]);

    // calculate ask fees and create message if applicable
    let ask_fee = match contract_info.ask_fee_info {
        // calculate ask fee using total
        Some(ask_fee_info) => {
            match Decimal::from_str(&ask_fee_info.rate)
                .map_err(|_| ContractError::InvalidFields {
                    fields: vec![String::from("ContractInfo.ask_fee_info.rate")],
                })?
                .checked_mul(actual_gross_proceeds)
                .ok_or(ContractError::TotalOverflow)?
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_u128()
                .ok_or(ContractError::TotalOverflow)?
            {
                0u128 => None,
                fee_total => {
                    let ask_fee = Coin {
                        denom: bid_order.quote.denom.to_owned(),
                        amount: Uint128::new(fee_total),
                    };

                    match is_quote_restricted_marker {
                        true => {
                            response = response.add_message(transfer_marker_coins(
                                fee_total,
                                bid_order.quote.denom.to_owned(),
                                ask_fee_info.account,
                                env.contract.address.to_owned(),
                            )?);
                        }
                        false => {
                            response = response.add_message(BankMsg::Send {
                                to_address: ask_fee_info.account.to_string(),
                                amount: vec![ask_fee.to_owned()],
                            });
                        }
                    }

                    // subtract the fees and assign to net proceeds
                    net_proceeds =
                        net_proceeds
                            .checked_sub(Uint128::new(fee_total))
                            .map_err(|error| {
                                ContractError::Std(StdError::Overflow { source: error })
                            })?;

                    Some(ask_fee)
                }
            }
        }
        None => None,
    };

    response = response.add_attribute(
        "ask_fee",
        match ask_fee {
            None => Uint128::zero(),
            Some(fee) => fee.amount,
        },
    );

    // get bid fees and create message if applicable
    let actual_bid_fee = match &bid_order.fee {
        Some(_) => bid_order.calculate_fee(Uint128::new(
            actual_gross_proceeds
                .to_u128()
                .ok_or(ContractError::TotalOverflow)?,
        ))?,
        None => None,
    };

    // add bid fee message
    match &actual_bid_fee {
        Some(bid_fee) => match contract_info.bid_fee_info {
            Some(bid_fee_info) => {
                match is_quote_restricted_marker {
                    true => {
                        response = response.add_message(transfer_marker_coins(
                            bid_fee.amount.to_owned().u128(),
                            bid_fee.denom.to_owned(),
                            bid_fee_info.account,
                            env.contract.address.to_owned(),
                        )?);
                    }
                    false => {
                        response = response.add_message(BankMsg::Send {
                            to_address: bid_fee_info.account.to_string(),
                            amount: vec![bid_fee.to_owned()],
                        });
                    }
                };
            }
            None => return Err(ContractError::BidFeeAccountMissing),
        },
        None => (),
    }

    response = response.add_attribute(
        "bid_fee",
        match &actual_bid_fee {
            None => Uint128::zero(),
            Some(fee) => fee.amount,
        },
    );

    // add 'send quote to asker' and 'send base to bidder' messages
    match &ask_order.class {
        AskOrderClass::Basic => {
            match is_quote_restricted_marker {
                true => {
                    response = response.add_message(transfer_marker_coins(
                        net_proceeds.into(),
                        bid_order.quote.denom.to_owned(),
                        ask_order.owner.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                }
                false => {
                    response = response.add_message(BankMsg::Send {
                        to_address: ask_order.owner.to_string(),
                        amount: vec![Coin {
                            denom: bid_order.quote.denom.to_owned(),
                            amount: net_proceeds,
                        }],
                    });
                }
            }
            match is_base_restricted_marker {
                true => {
                    response = response.add_message(transfer_marker_coins(
                        execute_size.into(),
                        ask_order.base.to_owned(),
                        bid_order.owner.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                }
                false => {
                    response = response.add_message(BankMsg::Send {
                        to_address: bid_order.owner.to_string(),
                        amount: vec![Coin {
                            denom: ask_order.base.clone(),
                            amount: execute_size,
                        }],
                    });
                }
            }
        }
        AskOrderClass::Convertible {
            status:
                AskOrderStatus::Ready {
                    approver,
                    converted_base,
                },
        } => {
            match is_base_restricted_marker {
                true => {
                    response = response.add_message(transfer_marker_coins(
                        execute_size.into(),
                        converted_base.to_owned().denom,
                        bid_order.owner.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                    response = response.add_message(transfer_marker_coins(
                        execute_size.into(),
                        ask_order.base.to_owned(),
                        approver.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                }
                false => {
                    response = response.add_message(BankMsg::Send {
                        to_address: bid_order.owner.to_owned().into(),
                        amount: vec![Coin {
                            denom: converted_base.to_owned().denom,
                            amount: execute_size,
                        }],
                    });
                    response = response.add_message(BankMsg::Send {
                        to_address: approver.to_string(),
                        amount: vec![Coin {
                            denom: ask_order.base.clone(),
                            amount: execute_size,
                        }],
                    });
                }
            }
            match is_quote_restricted_marker {
                true => {
                    response = response.add_message(transfer_marker_coins(
                        net_proceeds.into(),
                        bid_order.quote.denom.clone(),
                        approver.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                }
                false => {
                    response = response.add_message(BankMsg::Send {
                        to_address: approver.to_string(),
                        amount: vec![Coin {
                            denom: bid_order.quote.denom.clone(),
                            amount: net_proceeds,
                        }],
                    });
                }
            }
        }
        AskOrderClass::Convertible { status } => {
            return Err(ContractError::AskOrderNotReady {
                current_status: format!("{:?}", status),
            });
        }
    };

    // determine refunds to bidder
    if execute_price.lt(&bid_price) {
        // calculate gross proceeds using bid price, (price * size), error if overflows
        let original_gross_proceeds = bid_price
            .checked_mul(Decimal::from(execute_size.u128()))
            .ok_or(ContractError::TotalOverflow)?;

        // error if gross proceeds is not an integer
        if original_gross_proceeds.fract().ne(&Decimal::zero()) {
            return Err(ContractError::NonIntegerTotal);
        }

        // calculate refund
        let bid_quote_refund = original_gross_proceeds
            .checked_sub(actual_gross_proceeds)
            .ok_or(ContractError::TotalOverflow)?
            .to_u128()
            .ok_or(ContractError::NonIntegerTotal)?;

        // calculate fee based on original gross proceeds
        let bid_fee_refund = {
            let original_bid_fee = bid_order.calculate_fee(Uint128::new(
                original_gross_proceeds
                    .to_u128()
                    .ok_or(ContractError::TotalOverflow)?,
            ))?;

            match (&actual_bid_fee, original_bid_fee) {
                (Some(actual_bid_fee), Some(mut original_bid_fee)) => {
                    let refund_amount = original_bid_fee.amount - actual_bid_fee.amount;

                    if refund_amount.gt(&Uint128::zero()) {
                        original_bid_fee.amount = refund_amount;
                        Some(original_bid_fee)
                    } else {
                        None
                    }
                }
                (_, _) => None,
            }
        };

        if bid_quote_refund.gt(&0u128) {
            match is_quote_restricted_marker {
                true => {
                    // add the quote refund
                    response = response.add_message(transfer_marker_coins(
                        bid_quote_refund,
                        bid_order.quote.denom.to_owned(),
                        bid_order.owner.to_owned(),
                        env.contract.address.to_owned(),
                    )?);
                    // add the fee refund
                    if let Some(fee_refund) = &bid_fee_refund {
                        response = response.add_message(transfer_marker_coins(
                            fee_refund.amount.u128(),
                            fee_refund.denom.to_owned(),
                            bid_order.owner.to_owned(),
                            env.contract.address,
                        )?);
                    }
                }
                false => {
                    response = response.add_message(BankMsg::Send {
                        to_address: bid_order.owner.to_string(),
                        amount: vec![Coin {
                            denom: bid_order.quote.denom.clone(),
                            amount: bid_quote_refund.into(),
                        }],
                    });
                    if let Some(fee_refund) = &bid_fee_refund {
                        response = response.add_message(BankMsg::Send {
                            to_address: bid_order.owner.to_string(),
                            amount: vec![fee_refund.to_owned()],
                        });
                    }
                }
            }
        }

        // add fill event to bid order events
        bid_order.events.push(Event {
            action: Action::Fill {
                base: Coin {
                    denom: bid_order.base.denom.to_owned(),
                    amount: execute_size,
                },
                fee: actual_bid_fee,
                price,
                quote: Coin {
                    denom: bid_order.quote.denom.to_owned(),
                    amount: Uint128::new(
                        actual_gross_proceeds
                            .to_u128()
                            .ok_or(ContractError::TotalOverflow)?,
                    ),
                },
            },
            block_info: env.block.to_owned().into(),
        });
        // add refund event to bid order events
        bid_order.events.push(Event {
            action: Action::Refund {
                fee: bid_fee_refund,
                quote: Coin {
                    denom: bid_order.quote.denom.to_owned(),
                    amount: Uint128::new(bid_quote_refund),
                },
            },
            block_info: env.block.into(),
        });
    } else {
        // add fill event to bid order events
        bid_order.events.push(Event {
            action: Action::Fill {
                base: Coin {
                    denom: bid_order.base.denom.to_owned(),
                    amount: execute_size,
                },
                fee: actual_bid_fee,
                price,
                quote: Coin {
                    denom: bid_order.quote.denom.to_owned(),
                    amount: Uint128::new(
                        actual_gross_proceeds
                            .to_u128()
                            .ok_or(ContractError::TotalOverflow)?,
                    ),
                },
            },
            block_info: env.block.into(),
        });
    }

    // finally update or remove the orders from storage
    if ask_order.size.is_zero() {
        get_ask_storage(deps.storage).remove(ask_id.as_bytes());
    } else {
        get_ask_storage(deps.storage)
            .update(ask_id.as_bytes(), |_| -> StdResult<_> { Ok(ask_order) })?;
    }

    if bid_order.get_remaining_base().eq(&Uint128::zero()) {
        get_bid_storage(deps.storage).remove(bid_id.as_bytes());
    } else {
        get_bid_storage(deps.storage)
            .update(bid_id.as_bytes(), |_| -> StdResult<_> { Ok(bid_order) })?;
    }

    Ok(response)
}

// smart contract migrate/upgrade entrypoint
#[entry_point]
pub fn migrate(
    mut deps: DepsMut<ProvenanceQuery>,
    env: Env,
    msg: MigrateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    msg.validate()?;

    // build response
    let mut response: Response<ProvenanceMsg> = Response::new();

    // migrate contract_info
    migrate_contract_info(deps.branch(), &msg)?;

    // migrate ask orders
    migrate_ask_orders(deps.branch(), &msg)?;

    // migrate bid orders
    response = migrate_bid_orders(deps.branch(), env, &msg, response)?;

    // lastly, migrate version_info
    migrate_version_info(deps.branch())?;

    Ok(response)
}

// smart contract query entrypoint
#[entry_point]
pub fn query(deps: Deps<ProvenanceQuery>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
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
        Marker, NameMsgParams, ProvenanceMsg, ProvenanceMsgParams, ProvenanceRoute,
    };

    use crate::ask_order::{AskOrderClass, AskOrderV1};
    use crate::bid_order::get_bid_storage_read;

    use super::*;
    use crate::tests::test_setup_utils::setup_test_base_contract_v3;
    use provwasm_mocks::mock_dependencies;

    const QUOTE1_RESTRICTED_MARKER_JSON: &str = "{
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
                assert_eq!(init_response.messages.len(), 1);
                assert_eq!(
                    init_response.messages[0].msg,
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
                let expected_contract_info = ContractInfoV3 {
                    name: "contract_name".into(),
                    bind_name: "contract_bind_name".into(),
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
            bind_name: "".into(),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
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
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(500),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            size: Uint128::new(500),
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
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        500,
                        "base_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("asker")
                    )
                    .unwrap()
                );
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            size: Uint128::new(500),
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
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_1".to_string(),
            quote: "quote_1".into(),
            price: "2.5".into(),
            size: Uint128::new(200),
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
            size: Uint128::new(400),
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
                        size: Uint128::new(200)
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

        // create ask missing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "".into(),
            base: "".to_string(),
            quote: "".into(),
            price: "".into(),
            size: Uint128::new(0),
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

        // create ask with inconvertible base
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "inconvertible".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(100),
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

        // create ask with unsupported quote
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "unsupported".into(),
            price: "2".into(),
            size: Uint128::new(100),
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
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create ask
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2.123".into(),
            size: Uint128::new(500),
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

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            base: "base_denom".to_string(),
            quote: "quote_1".into(),
            price: "2".into(),
            size: Uint128::new(200),
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
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
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
            fee: None,
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(response.attributes[3], attr("fee", "None"));
                assert_eq!(response.attributes[4], attr("price", "2.5"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "250"));
                assert_eq!(response.attributes[7], attr("size", "100"));
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
            fee,
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
                        BidOrderV2 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
    fn create_bid_with_restricted_marker_valid_data() {
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            fee: None,
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(1000),
            size: Uint128::new(500),
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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(response.attributes[3], attr("fee", "None"));
                assert_eq!(response.attributes[4], attr("price", "2"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "1000"));
                assert_eq!(response.attributes[7], attr("size", "500"));

                assert_eq!(response.messages.len(), 1);
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        1000,
                        "quote_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("bidder")
                    )
                    .unwrap()
                );
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
            fee,
            price,
            quote,
            quote_size,
            size,
        } = create_bid_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV2 {
                            id,
                            owner: bidder_info.sender,
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
    fn create_bid_with_fees_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            base: "base_1".into(),
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            fee: Some(Coin {
                amount: Uint128::new(25),
                denom: "quote_1".into(),
            }),
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
        };

        let bidder_info = mock_info("bidder", &coins(275, "quote_1"));

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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(25),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "2.5"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "250"));
                assert_eq!(response.attributes[7], attr("size", "100"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
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
                        BidOrderV2 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
    fn create_bid_with_fees_round_down_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
            fee: Some(Coin {
                amount: Uint128::new(1),
                denom: "quote_1".into(),
            }),
            price: "1".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(149),
            size: Uint128::new(149),
        };

        let bidder_info = mock_info("bidder", &coins(150, "quote_1"));

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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(1),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "149"));
                assert_eq!(response.attributes[7], attr("size", "149"));
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
            fee,
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
                        BidOrderV2 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
    fn create_bid_with_fees_round_up_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
            fee: Some(Coin {
                amount: Uint128::new(2),
                denom: "quote_1".into(),
            }),
            price: "1".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(150),
            size: Uint128::new(150),
        };

        let bidder_info = mock_info("bidder", &coins(152, "quote_1"));

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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(2),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "1"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "150"));
                assert_eq!(response.attributes[7], attr("size", "150"));
            }
            Err(error) => {
                panic!("failed to create bid: {:?}", error)
            }
        }

        // verify bid order stored
        let bid_storage = get_bid_storage_read(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
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
                        BidOrderV2 {
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
    fn create_bid_with_restricted_marker_with_fees_valid_data() {
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_acct"),
                    rate: "0.1".into(),
                }),
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            base: "base_1".to_string(),
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            fee: Some(Coin {
                amount: Uint128::new(100),
                denom: "quote_1".into(),
            }),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(1000),
            size: Uint128::new(500),
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
                assert_eq!(response.attributes.len(), 8);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("base", "base_1"));
                assert_eq!(
                    response.attributes[2],
                    attr("id", "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367")
                );
                assert_eq!(
                    response.attributes[3],
                    attr(
                        "fee",
                        format!(
                            "{:?}",
                            Coin {
                                amount: Uint128::new(100),
                                denom: "quote_1".into(),
                            }
                        )
                    )
                );
                assert_eq!(response.attributes[4], attr("price", "2"));
                assert_eq!(response.attributes[5], attr("quote", "quote_1"));
                assert_eq!(response.attributes[6], attr("quote_size", "1000"));
                assert_eq!(response.attributes[7], attr("size", "500"));

                assert_eq!(response.messages.len(), 1);
                assert_eq!(
                    response.messages[0].msg,
                    transfer_marker_coins(
                        1100,
                        "quote_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("bidder")
                    )
                    .unwrap()
                );
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify bid order stored
        let ask_storage = get_bid_storage_read(&deps.storage);
        if let ExecuteMsg::CreateBid {
            base,
            fee,
            id,
            price,
            quote,
            quote_size,
            size,
        } = create_bid_msg
        {
            match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrderV2 {
                            id,
                            owner: bidder_info.sender,
                            base: Coin {
                                amount: size,
                                denom: base,
                            },
                            events: vec![],
                            fee,
                            price,
                            quote: Coin {
                                amount: quote_size,
                                denom: quote,
                            },
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
            base: "base_1".to_string(),
            fee: None,
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(10),
            size: Uint128::new(500),
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
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
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

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_1".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(250),
            size: Uint128::new(100),
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
            base: "base_1".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4.5".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(900),
            size: Uint128::new(200),
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
                    BidOrderV2 {
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        events: vec![],
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2.5".into(),
                        quote: Coin {
                            amount: Uint128::new(250),
                            denom: "quote_1".into(),
                        },
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

        // create bid missing id
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "".into(),
            fee: None,
            id: "".into(),
            price: "".into(),
            quote: "".into(),
            quote_size: Uint128::new(0),
            size: Uint128::new(0),
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

        // create bid with invalid base
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "notbasedenom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_2".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
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

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "unsupported".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
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
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(100),
            size: Uint128::new(100),
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

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
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
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.123".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200),
            size: Uint128::new(100),
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
    fn create_bid_restricted_quote_denom_and_quote_mismatch_order_amount_and_size() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base_contract_v3(&mut deps.storage);

        let test_marker: Marker =
            from_binary(&Binary::from(QUOTE1_RESTRICTED_MARKER_JSON.as_bytes())).unwrap();
        deps.querier.with_markers(vec![test_marker]);

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            base: "base_denom".into(),
            fee: None,
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "3".into(),
            quote: "quote_1".into(),
            quote_size: Uint128::new(200), // Valid amount would be 300
            size: Uint128::new(100),
        };

        // execute create bid
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &[]),
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
    fn execute_valid_data() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }


    #[test]
    fn execute_quote_denom_mismatch_returns_err() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_2".into() // not equal to "quote_1"
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("exec_1", &[]),
            execute_msg,
        );

        // validate execute response
        match execute_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedQuoteDenom => {  }
                error => panic!("unexpected error: {:?}", error),
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }

    #[test]
    fn execute_with_ask_fees_round_down() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "1"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "ask_fee_account".into(),
                        amount: coins(1, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(148, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_ask_fees_round_up() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(150),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(150),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(150),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(150),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "150"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "2"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "ask_fee_account".into(),
                        amount: coins(2, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(148, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(150, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_round_down() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: Some(Coin {
                    amount: Uint128::new(1),
                    denom: "quote_1".into(),
                }),
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "1"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: coins(1, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(149, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_not_applicable() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(149),
            },
        );

        // store valid bid order without fees
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(149),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(149),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(149),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "149"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(149, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(149, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_with_bid_fees_round_up() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.01".into(),
                }),
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(0),
                size_increment: Uint128::new(1),
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
                price: "1".into(),
                quote: "quote_1".into(),
                size: Uint128::new(150),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(150),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: Some(Coin {
                    amount: Uint128::new(2),
                    denom: "quote_1".into(),
                }),
                quote: Coin {
                    amount: Uint128::new(150),
                    denom: "quote_1".into(),
                },
                price: "1".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "1".into(),
            size: Uint128::new(150),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[5], attr("price", "1"));
                assert_eq!(execute_response.attributes[6], attr("size", "150"));
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "2"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: coins(2, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(150, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(150, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_ask_order() {
        // setup
        let mut deps = mock_dependencies(&[coin(30, "base_1"), coin(20, "quote_1")]);
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
                size: Uint128::new(30),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(20),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(10),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(20, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
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
                        size: Uint128::new(20)
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_bid_order() {
        // setup
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(50),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(200),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(50),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(100, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
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
                    BidOrderV2 {
                        base: Coin {
                            amount: Uint128::new(100),
                            denom: "base_1".into(),
                        },
                        events: vec![Event {
                            action: Action::Fill {
                                base: Coin {
                                    denom: "base_1".to_string(),
                                    amount: Uint128::new(50)
                                },
                                fee: None,
                                price: "2".to_string(),
                                quote: Coin {
                                    denom: "quote_1".to_string(),
                                    amount: Uint128::new(100)
                                },
                            },
                            block_info: mock_env().block.into(),
                        }],
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(200),
                            denom: "quote_1".into(),
                        },
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_partial_both_orders() {
        // setup
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(300),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(600),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(200, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
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
                        size: Uint128::new(100)
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
                    BidOrderV2 {
                        base: Coin {
                            amount: Uint128::new(300),
                            denom: "base_1".into(),
                        },
                        events: vec![Event {
                            action: Action::Fill {
                                base: Coin {
                                    denom: "base_1".to_string(),
                                    amount: Uint128::new(100)
                                },
                                fee: None,
                                price: "2".to_string(),
                                quote: Coin {
                                    denom: "quote_1".to_string(),
                                    amount: Uint128::new(200)
                                },
                            },
                            block_info: mock_env().block.into(),
                        }],
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(600),
                            denom: "quote_1".into(),
                        },
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
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(300),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(600),
                    denom: "quote_1".into(),
                },
                price: "2".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(100, "con_base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_2".into(),
                        amount: vec![coin(200, "quote_1")],
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
                        size: Uint128::new(100)
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
                    BidOrderV2 {
                        base: Coin {
                            amount: Uint128::new(300),
                            denom: "base_1".into(),
                        },
                        events: vec![Event {
                            action: Action::Fill {
                                base: Coin {
                                    denom: "base_1".to_string(),
                                    amount: Uint128::new(100)
                                },
                                fee: None,
                                price: "2".to_string(),
                                quote: Coin {
                                    denom: "quote_1".to_string(),
                                    amount: Uint128::new(200)
                                },
                            },
                            block_info: mock_env().block.into(),
                        }],
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "2".into(),
                        quote: Coin {
                            amount: Uint128::new(600),
                            denom: "quote_1".into(),
                        },
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // difference
    #[test]
    fn execute_price_overlap_use_ask() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(5),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(500),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // difference
    #[test]
    fn execute_price_overlap_use_ask_with_partial_bid() {
        // setup
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
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                quote: Coin {
                    amount: Uint128::new(500),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order update
        let bid_storage = get_bid_storage_read(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV2 {
                        base: Coin {
                            amount: Uint128::new(10),
                            denom: "base_1".into(),
                        },
                        events: vec![
                            Event {
                                action: Action::Fill {
                                    base: Coin {
                                        denom: "base_1".to_string(),
                                        amount: Uint128::new(5),
                                    },
                                    fee: None,
                                    price: "2.000000000000000000".to_string(),
                                    quote: Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(10),
                                    },
                                },
                                block_info: mock_env().block.into(),
                            },
                            Event {
                                action: Action::Refund {
                                    fee: None,
                                    quote: Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(490),
                                    },
                                },
                                block_info: mock_env().block.into(),
                            }
                        ],
                        fee: None,
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        price: "100.000000000000000000".into(),
                        quote: Coin {
                            amount: Uint128::new(500),
                            denom: "quote_1".into(),
                        },
                    }
                )
            }
            _ => {
                panic!("bid order was not found in storage")
            }
        }
    }

    // since using ask price, and ask.price < bid.price, bidder should be refunded
    // partial quote and partial fee
    #[test]
    fn execute_price_overlap_use_ask_with_bid_fees() {
        // setup
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
                bid_fee_info: Some(FeeInfo {
                    account: Addr::unchecked("bid_fee_account"),
                    rate: "0.1".to_string(),
                }),
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
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(10),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: Some(Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".to_string(),
                }),
                quote: Coin {
                    amount: Uint128::new(1000),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "1"));

                assert_eq!(execute_response.messages.len(), 5);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bid_fee_account".into(),
                        amount: vec![coin(1, "quote_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(10, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[3].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(490, "quote_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[4].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(49, "quote_1")],
                    })
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        match bid_storage.load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrderV2 {
                        id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                        owner: Addr::unchecked("bidder"),
                        base: Coin {
                            amount: Uint128::new(10),
                            denom: "base_1".into(),
                        },
                        events: vec![
                            Event {
                                action: Action::Fill {
                                    base: Coin {
                                        denom: "base_1".to_string(),
                                        amount: Uint128::new(5),
                                    },
                                    fee: Some(Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(1),
                                    }),
                                    price: "2.000000000000000000".to_string(),
                                    quote: Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(10),
                                    },
                                },
                                block_info: mock_env().block.into(),
                            },
                            Event {
                                action: Action::Refund {
                                    fee: Some(Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(49),
                                    }),
                                    quote: Coin {
                                        denom: "quote_1".to_string(),
                                        amount: Uint128::new(490),
                                    },
                                },
                                block_info: mock_env().block.into(),
                            }
                        ],
                        fee: Some(Coin {
                            amount: Uint128::new(100),
                            denom: "quote_1".to_string(),
                        }),
                        quote: Coin {
                            denom: "quote_1".into(),
                            amount: Uint128::new(1000),
                        },
                        price: "100.000000000000000000".into(),
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
    fn execute_price_overlap_use_ask_and_quote_restricted() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                price: "2.000000000000000000".into(),
                quote: "quote_1".into(),
                size: Uint128::new(777),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(5),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(500),
                    denom: "quote_1".into(),
                },
                price: "100.000000000000000000".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2.000000000000000000".into(),
            size: Uint128::new(5),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        10,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(5, "base_1")],
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        490,
                        "quote_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order IS NOT removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_price_overlap_use_bid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
                price: "4".into(),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: coins(400, "quote_1"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_1"),
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: coins(100, "base_denom"),
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(100, "con_base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_ask() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "asker".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_bid() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(100, "base_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_restricted_marker_ask_and_bid() {
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 2);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("asker"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_base_restricted_marker() {
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(400, "quote_1")]
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_quote_restricted_marker() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();

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

        let marker_quote_1: Marker = from_binary(&Binary::from(quote_marker_json)).unwrap();
        deps.querier.with_markers(vec![marker_quote_1]);

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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,
                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "bidder".into(),
                        amount: vec![coin(100, "base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: "approver_1".into(),
                        amount: vec![coin(100, "con_base_1")]
                    })
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_convertible_with_base_and_quote_restricted_marker() {
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

        let restricted_quote_marker_json = b"{
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

        let marker_base_1: Marker = from_binary(&Binary::from(restricted_base_1)).unwrap();
        let marker_con_base_1: Marker = from_binary(&Binary::from(restricted_con_base_1)).unwrap();
        let marker_quote_1: Marker =
            from_binary(&Binary::from(restricted_quote_marker_json)).unwrap();
        deps.querier
            .with_markers(vec![marker_base_1, marker_con_base_1, marker_quote_1]);

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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(100),
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
                assert_eq!(execute_response.attributes.len(), 9);
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
                assert_eq!(execute_response.attributes[7], attr("ask_fee", "0"));
                assert_eq!(execute_response.attributes[8], attr("bid_fee", "0"));

                assert_eq!(execute_response.messages.len(), 3);
                assert_eq!(
                    execute_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked("bidder"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[1].msg,
                    transfer_marker_coins(
                        100,
                        "con_base_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
                assert_eq!(
                    execute_response.messages[2].msg,
                    transfer_marker_coins(
                        400,
                        "quote_1",
                        Addr::unchecked("approver_1"),
                        Addr::unchecked(MOCK_CONTRACT_ADDR)
                    )
                    .unwrap()
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_err());

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_err());
    }

    #[test]
    fn execute_invalid_data() {
        // setup
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

        // execute by non-executor
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(1),
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
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                owner: Addr::unchecked("asker"),
                price: "2".into(),
                quote: "quote_1".into(),
                size: Uint128::new(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute when ask order not ready returns ContractError::PendingIssuerApproval
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on non-existent ask order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
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
                size: Uint128::new(200),
            },
        );

        // execute on non-existent bid order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
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
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                approvers: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                ask_fee_info: None,
                bid_fee_info: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                supported_quote_denoms: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
            },
        );

        // execute with sent_funds returns ContractError::ExecuteWithFunds
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(1),
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
                price: "3".into(),
                quote: "quote_1".into(),
                size: Uint128::new(300),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(200),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "2".into(),
                quote: Coin {
                    amount: Uint128::new(100),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "2".into(),
            size: Uint128::new(200),
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "6".into(),
            size: Uint128::new(100),
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
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order still exists
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }

    #[test]
    fn execute_size_greater_than_ask_and_bid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrderV2 {
                base: Coin {
                    amount: Uint128::new(100),
                    denom: "base_1".into(),
                },
                events: vec![],
                fee: None,

                id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
                owner: Addr::unchecked("bidder"),
                price: "4".into(),
                quote: Coin {
                    amount: Uint128::new(400),
                    denom: "quote_1".into(),
                },
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            bid_id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            price: "4".into(),
            size: Uint128::new(200),
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
        assert!(ask_storage
            .load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes())
            .is_ok());

        // verify bid order still exists
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert!(bid_storage
            .load("c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".as_bytes())
            .is_ok());
    }

    #[test]
    fn execute_modify_contract_valid() {
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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

        deps.querier.with_attributes(
            "asker",
            &[
                ("ask_tag_1", "ask_tag_1_value", "String"),
                ("ask_tag_2", "ask_tag_2_value", "String"),
            ],
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

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
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
        let mut deps = mock_dependencies(&[]);

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

        deps.querier.with_attributes(
            "asker",
            &[
                ("ask_tag_1", "ask_tag_1_value", "String"),
                ("ask_tag_2", "ask_tag_2_value", "String"),
            ],
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
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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

        deps.querier.with_attributes(
            "asker",
            &[
                ("ask_tag_1", "ask_tag_1_value", "String"),
                ("ask_tag_2", "ask_tag_2_value", "String"),
            ],
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
        let mut deps = mock_dependencies(&[]);

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
        let mut deps = mock_dependencies(&[]);

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

        deps.querier.with_attributes(
            "asker",
            &[
                ("ask_tag_1", "ask_tag_1_value", "String"),
                ("ask_tag_2", "ask_tag_2_value", "String"),
            ],
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

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
            ],
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
        let mut deps = mock_dependencies(&[]);

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

        deps.querier.with_attributes(
            "bidder",
            &[
                ("bid_tag_1", "bid_tag_1_value", "String"),
                ("bid_tag_2", "bid_tag_2_value", "String"),
                ("bid_tag_3", "bid_tag_3_value", "String"),
                ("bid_tag_4", "bid_tag_4_value", "String"),
            ],
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

    #[test]
    fn approve_ask_valid() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
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
                        size: Uint128::new(100),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(100),
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
                assert_eq!(
                    approve_ask_response.messages[0].msg,
                    transfer_marker_coins(
                        100,
                        "base_1",
                        Addr::unchecked(MOCK_CONTRACT_ADDR),
                        Addr::unchecked("approver_1")
                    )
                    .unwrap()
                );
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
                        size: Uint128::new(100),
                    }
                )
            }
            _ => {
                panic!("ask order was not found in storage")
            }
        }
    }

    #[test]
    fn approve_ask_already_approved_return_err() {
        // setup
        let mut deps = mock_dependencies(&[]);
        let mock_env = mock_env();
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

        // store approved ask order
        let existing_ask_order = AskOrderV1 {
            base: "con_base_1".into(),
            class: AskOrderClass::Convertible {
                status: AskOrderStatus::Ready {
                    // Already marked ready
                    approver: Addr::unchecked("approver_1"),
                    converted_base: coin(100, "base_denom"),
                },
            },
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        store_test_ask(&mut deps.storage, &existing_ask_order);

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_2", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
            },
        );

        // validate ask response
        match approve_ask_response {
            Err(error) => match error {
                ContractError::AskOrderReady { approver } => {
                    assert_eq!("approver_1", approver)
                }
                _ => panic!("unexpected error type: {:?}", error),
            },
            Ok(_) => panic!("expected error, but ok"),
        }

        // verify ask order the same
        let ask_storage = get_ask_storage_read(&deps.storage);
        match ask_storage.load("ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(stored_order, existing_ask_order)
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "59e82f8f-268e-433f-9711-e9f2d2cc19a5".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
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
                        size: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "wrong_base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "wrong_base_denom".to_string(),
                size: Uint128::new(100),
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
                        size: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(101, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
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
                        size: Uint128::new(100),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(99),
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
                        size: Uint128::new(100),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[coin(10, "gme")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(100),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("approver_1", &[]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_1".into(),
                size: Uint128::new(101),
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
                        size: Uint128::new(100),
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
                ask_required_attributes: vec![],
                bid_required_attributes: vec![],
                price_precision: Uint128::new(2),
                size_increment: Uint128::new(100),
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
                size: Uint128::new(100),
            },
        );

        let approve_ask_response = execute(
            deps.as_mut(),
            mock_env,
            mock_info("not_approver", &[coin(100, "base_denom")]),
            ExecuteMsg::ApproveAsk {
                id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
                base: "base_denom".to_string(),
                size: Uint128::new(100),
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
                        size: Uint128::new(100),
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

    fn setup_test_base(storage: &mut dyn Storage, contract_info: &ContractInfoV3) {
        if let Err(error) = set_contract_info(storage, contract_info) {
            panic!("unexpected error: {:?}", error)
        }
    }

    fn store_test_ask(storage: &mut dyn Storage, ask_order: &AskOrderV1) {
        let mut ask_storage = get_ask_storage(storage);
        if let Err(error) = ask_storage.save(ask_order.id.as_bytes(), ask_order) {
            panic!("unexpected error: {:?}", error)
        };
    }

    fn store_test_bid(storage: &mut dyn Storage, bid_order: &BidOrderV2) {
        let mut bid_storage = get_bid_storage(storage);
        if let Err(error) = bid_storage.save(bid_order.id.as_bytes(), bid_order) {
            panic!("unexpected error: {:?}", error);
        };
    }
}
