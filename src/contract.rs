use crate::ask_order::{migrate_ask_orders, AskOrderClass, AskOrderStatus, AskOrderV1, ASKS_V1};
use crate::bid_order::{migrate_bid_orders, BidOrderV3, BIDS_V3};
use crate::common::{Action, ContractAction, FeeInfo};
use crate::contract_info::{
    get_contract_info, migrate_contract_info, set_contract_info, ContractInfoV3,
};
use crate::error::ContractError;
use crate::error::ContractError::InvalidPricePrecisionSizePair;
use crate::execute::modify_contract::modify_contract;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, Validate};
use crate::util::{
    add_transfer, get_attributes, is_invalid_price_precision, is_restricted_marker,
    transfer_marker_coins,
};
use crate::version_info::{
    get_version_info, migrate_version_info, set_version_info, VersionInfoV1, CRATE_NAME,
    PACKAGE_VERSION,
};
use cosmwasm_std::{
    attr, coin, coins, entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps,
    DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128,
};
use provwasm_std::types::provenance::attribute::v1::AttributeQuerier;
use rust_decimal::prelude::{FromPrimitive, FromStr, ToPrimitive, Zero};
use rust_decimal::{Decimal, RoundingStrategy};
use std::cmp::Ordering;
use std::collections::HashSet;

// smart contract initialization entrypoint
#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
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
        bind_name: "".into(),
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

    set_version_info(
        deps.storage,
        &VersionInfoV1 {
            version: PACKAGE_VERSION.to_string(),
            definition: CRATE_NAME.to_string(),
        },
    )?;

    // build response
    Ok(Response::new().add_attributes(vec![
        attr(
            "contract_info",
            format!("{:?}", get_contract_info(deps.storage)?),
        ),
        attr("action", ContractAction::Init.to_string()),
    ]))
}

// smart contract execute entrypoint
#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
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
            BidOrderV3 {
                base: Coin {
                    amount: size,
                    denom: base,
                },
                accumulated_base: Uint128::zero(),
                accumulated_quote: Uint128::zero(),
                accumulated_fee: Uint128::zero(),
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
            reverse_bid(deps, env, info, id, ContractAction::CancelBid, None)
        }
        ExecuteMsg::ExecuteMatch {
            ask_id,
            bid_id,
            price,
            size,
        } => execute_match(deps, env, info, ask_id, bid_id, price, size),
        ExecuteMsg::ExpireAsk { id } => {
            reverse_ask(deps, env, info, id, ContractAction::ExpireAsk, None)
        }
        ExecuteMsg::ExpireBid { id } => {
            reverse_bid(deps, env, info, id, ContractAction::ExpireBid, None)
        }
        ExecuteMsg::RejectAsk { id, size } => {
            reverse_ask(deps, env, info, id, ContractAction::RejectAsk, size)
        }
        ExecuteMsg::RejectBid { id, size } => {
            reverse_bid(deps, env, info, id, ContractAction::RejectBid, size)
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
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
    base: String,
    size: Uint128,
) -> Result<Response, ContractError> {
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

    // update ask order
    let updated_ask_order = ASKS_V1.update(
        deps.storage,
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
                            AskOrderStatus::PendingIssuerApproval {} => {}
                        },
                        AskOrderClass::Basic => return Err(ContractError::InconvertibleBaseDenom),
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
        attr("action", ContractAction::ApproveAsk.to_string()),
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
            env.contract.address.to_owned(),
            info.sender,
            env.contract.address,
        )?);
    }

    Ok(response)
}

// create ask entrypoint
fn create_ask(
    deps: DepsMut,
    env: Env,
    info: &MessageInfo,
    mut ask_order: AskOrderV1,
) -> Result<Response, ContractError> {
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

    if ask_price.is_zero() || ask_price.is_sign_negative() {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

    // error if price smaller than allow price precision
    if is_invalid_price_precision(ask_price.clone(), contract_info.price_precision.clone()) {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

    // error if asker does not have required account attributes
    if !contract_info.ask_required_attributes.is_empty() {
        let querier = AttributeQuerier::new(&deps.querier);
        let attributes = get_attributes(info.sender.to_string(), &querier)?;
        let attributes_names: HashSet<String> =
            attributes.into_iter().map(|item| item.name).collect();
        if contract_info
            .ask_required_attributes
            .iter()
            .any(|item| !attributes_names.contains(item))
        {
            return Err(ContractError::Unauthorized);
        }
    }

    if ask_order.base.ne(&contract_info.base_denom) {
        ask_order.class = AskOrderClass::Convertible {
            status: AskOrderStatus::PendingIssuerApproval,
        };
    };

    if ASKS_V1
        .may_load(deps.storage, ask_order.id.as_bytes())?
        .is_some()
    {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("id")],
        });
    }

    ASKS_V1.save(deps.storage, ask_order.id.as_bytes(), &ask_order)?;

    let mut response = Response::new().add_attributes(vec![
        attr("action", ContractAction::CreateAsk.to_string()),
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
            env.contract.address.to_owned(),
            ask_order.owner,
            env.contract.address,
        )?);
    }

    Ok(response)
}

// create bid entrypoint
fn create_bid(
    deps: DepsMut,
    env: Env,
    info: &MessageInfo,
    mut bid_order: BidOrderV3,
) -> Result<Response, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    let bid_price =
        Decimal::from_str(&bid_order.price).map_err(|_| ContractError::InvalidFields {
            fields: vec![String::from("price")],
        })?;

    if bid_price.is_zero() || bid_price.is_sign_negative() {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("price")],
        });
    }

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

    // Get the bid fee rate (0 if not set)
    let bid_fee_rate = match contract_info.bid_fee_info {
        Some(bid_fee_info) => {
            Decimal::from_str(&bid_fee_info.rate).map_err(|_| ContractError::InvalidFields {
                fields: vec![String::from("ContractInfo.bid_fee_info.rate")],
            })?
        }
        None => Decimal::from(0),
    };

    // Calculate the expected fees (bid_fee_rate * total)
    let calculated_fee_size = bid_fee_rate
        .checked_mul(total)
        .ok_or(ContractError::TotalOverflow)?
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_u128()
        .ok_or(ContractError::TotalOverflow)?;

    match &mut bid_order.fee {
        Some(fee) => {
            // If the user sent fees, then make sure the amount + denom match
            if fee.amount.ne(&Uint128::new(calculated_fee_size)) {
                return Err(ContractError::InvalidFeeSize {
                    fee_rate: bid_fee_rate.to_string(),
                });
            }
            if fee.denom.ne(&bid_order.quote.denom) {
                return Err(ContractError::SentFundsOrderMismatch);
            }
        }
        None => {
            // If the user did not send fees, make sure the calculated fees was 0
            if calculated_fee_size.ne(&0) {
                return Err(ContractError::InvalidFeeSize {
                    fee_rate: bid_fee_rate.to_string(),
                });
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
        let querier = AttributeQuerier::new(&deps.querier);
        let attributes = get_attributes(info.sender.to_string(), &querier)?;
        let attributes_names: HashSet<String> =
            attributes.into_iter().map(|item| item.name).collect();
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

    if BIDS_V3
        .may_load(deps.storage, bid_order.id.as_bytes())?
        .is_some()
    {
        return Err(ContractError::InvalidFields {
            fields: vec![String::from("id")],
        });
    }

    BIDS_V3.save(deps.storage, bid_order.id.as_bytes(), &bid_order)?;

    let mut response = Response::new().add_attributes(vec![
        attr("action", ContractAction::CreateBid.to_string()),
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
            env.contract.address.to_owned(),
            bid_order.owner,
            env.contract.address,
        )?);
    }

    Ok(response)
}

// cancel ask entrypoint
fn cancel_ask(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
) -> Result<Response, ContractError> {
    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::CancelWithFunds);
    }

    let AskOrderV1 {
        id,
        owner,
        class,
        base,
        size,
        ..
    } = ASKS_V1
        .load(deps.storage, id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;
    if !info.sender.eq(&owner) {
        return Err(ContractError::Unauthorized);
    }

    // remove the ask order from storage
    ASKS_V1.remove(deps.storage, id.as_bytes());

    // is ask base a marker
    let is_base_restricted_marker = is_restricted_marker(&deps.querier, base.clone());

    let mut response = Response::new();

    // return 'base' to owner, return converted_base to issuer if applicable
    response = add_transfer(
        response,
        is_base_restricted_marker,
        size.into(),
        base,
        owner,
        env.contract.address.to_owned(),
        env.contract.address.to_owned(),
    );

    response = response.add_attributes(vec![
        attr("action", ContractAction::CancelAsk.to_string()),
        attr("id", id),
    ]);

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

        response = add_transfer(
            response,
            is_convertible_restricted_marker,
            converted_base.amount.into(),
            converted_base.denom,
            approver,
            env.contract.address.to_owned(),
            env.contract.address,
        );
    }

    Ok(response)
}

// reverse ask entrypoint
fn reverse_ask(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
    action: ContractAction,
    cancel_size: Option<Uint128>,
) -> Result<Response, ContractError> {
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

    // retrieve the order
    let mut ask_order = ASKS_V1
        .load(deps.storage, id.as_bytes())
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
    let is_base_restricted_marker = is_restricted_marker(&deps.querier, ask_order.base.clone());

    let mut response = Response::new();

    // return 'base' to owner, return converted_base to issuer if applicable
    response = add_transfer(
        response,
        is_base_restricted_marker,
        effective_cancel_size.into(),
        ask_order.base.to_owned(),
        ask_order.owner.to_owned(),
        env.contract.address.to_owned(),
        env.contract.address.to_owned(),
    );

    response = response.add_attributes(vec![
        attr("action", action.to_string()),
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

        response = add_transfer(
            response,
            is_convertible_restricted_marker,
            effective_cancel_size.into(),
            converted_base.denom,
            approver,
            env.contract.address.to_owned(),
            env.contract.address,
        );
    }

    // remove the ask order from storage if remaining size is 0, otherwise, store updated order
    if ask_order.size.is_zero() {
        ASKS_V1.remove(deps.storage, ask_order.id.as_bytes());
        response = response.add_attributes(vec![attr("order_open", "false")]);
    } else {
        ASKS_V1.save(deps.storage, ask_order.id.as_bytes(), &ask_order)?;
        response = response.add_attributes(vec![attr("order_open", "true")]);
    }

    Ok(response)
}

// reverse bid entrypoint
fn reverse_bid(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
    action: ContractAction,
    cancel_size: Option<Uint128>,
) -> Result<Response, ContractError> {
    // return error if id is empty
    if id.is_empty() {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::ExpireWithFunds);
    }

    let contract_info = get_contract_info(deps.storage)?;

    //load the bid order
    let mut bid_order = BIDS_V3
        .load(deps.storage, id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    if action.eq(&ContractAction::CancelBid) {
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
        .checked_mul(Decimal::from(effective_cancel_size.u128()))
        .ok_or(ContractError::TotalOverflow)?;

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
                .checked_mul(quote_remaining_ratio)
                .ok_or(ContractError::TotalOverflow)?
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

    bid_order.update_remaining_amounts(&Action::Reject {
        base: Coin {
            amount: effective_cancel_size,
            denom: bid_order.base.denom.to_owned(),
        },
        fee: effective_cancel_fee_size.to_owned(),
        quote: Coin {
            amount: effective_cancel_quote_size,
            denom: bid_order.quote.denom.to_owned(),
        },
    })?;

    let mut response = Response::new();

    // 'send quote back to owner' message
    response = add_transfer(
        response,
        is_quote_restricted_marker.to_owned(),
        effective_cancel_quote_size.u128(),
        bid_order.quote.denom.to_owned(),
        bid_order.owner.to_owned(),
        env.contract.address.to_owned(),
        env.contract.address.to_owned(),
    );

    response = response.add_attributes(vec![
        attr("action", action.to_string()),
        attr("id", id),
        attr("reverse_size", effective_cancel_size),
    ]);

    // add 'send fee back to owner' message
    if let Some(fee) = effective_cancel_fee_size {
        if fee.amount.gt(&Uint128::zero()) {
            response = add_transfer(
                response,
                is_quote_restricted_marker,
                fee.amount.u128(),
                bid_order.quote.denom.to_owned(),
                bid_order.owner.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address,
            );
        }
    }

    // remove the bid order from storage if remaining size is 0, otherwise, store updated order
    match bid_order.get_remaining_base().is_zero() {
        true => {
            BIDS_V3.remove(deps.storage, bid_order.id.as_bytes());
            response = response.add_attributes(vec![attr("order_open", "false")]);
        }
        false => {
            BIDS_V3.save(deps.storage, bid_order.id.as_bytes(), &bid_order)?;
            response = response.add_attributes(vec![attr("order_open", "true")]);
        }
    }

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
) -> Result<Response, ContractError> {
    let contract_info = get_contract_info(deps.storage)?;

    // only executors may execute matches
    if !contract_info.executors.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    // return error if funds sent
    if !info.funds.is_empty() {
        return Err(ContractError::ExecuteWithFunds);
    }

    let mut ask_order = ASKS_V1
        .load(deps.storage, ask_id.as_bytes())
        .map_err(|error| ContractError::LoadOrderFailed { error })?;

    let mut bid_order = BIDS_V3
        .load(deps.storage, bid_id.as_bytes())
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
        attr("action", ContractAction::Execute.to_string()),
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

                    response = add_transfer(
                        response,
                        is_quote_restricted_marker.to_owned(),
                        fee_total,
                        bid_order.quote.denom.to_owned(),
                        ask_fee_info.account,
                        env.contract.address.to_owned(),
                        env.contract.address.to_owned(),
                    );

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
                response = add_transfer(
                    response,
                    is_quote_restricted_marker.to_owned(),
                    bid_fee.amount.to_owned().u128(),
                    bid_fee.denom.to_owned(),
                    bid_fee_info.account,
                    env.contract.address.to_owned(),
                    env.contract.address.to_owned(),
                );
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
            response = add_transfer(
                response,
                is_quote_restricted_marker.to_owned(),
                net_proceeds.into(),
                bid_order.quote.denom.to_owned(),
                ask_order.owner.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );
            response = add_transfer(
                response,
                is_base_restricted_marker.to_owned(),
                execute_size.into(),
                ask_order.base.to_owned(),
                bid_order.owner.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );
        }
        AskOrderClass::Convertible {
            status:
                AskOrderStatus::Ready {
                    approver,
                    converted_base,
                },
        } => {
            response = add_transfer(
                response,
                is_base_restricted_marker.to_owned(),
                execute_size.into(),
                converted_base.to_owned().denom,
                bid_order.owner.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );
            response = add_transfer(
                response,
                is_base_restricted_marker,
                execute_size.into(),
                ask_order.base.to_owned(),
                approver.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );

            response = add_transfer(
                response,
                is_quote_restricted_marker.to_owned(),
                net_proceeds.into(),
                bid_order.quote.denom.clone(),
                approver.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );
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
            response = add_transfer(
                response,
                is_quote_restricted_marker.to_owned(),
                bid_quote_refund,
                bid_order.quote.denom.to_owned(),
                bid_order.owner.to_owned(),
                env.contract.address.to_owned(),
                env.contract.address.to_owned(),
            );
            if let Some(fee_refund) = &bid_fee_refund {
                response = add_transfer(
                    response,
                    is_quote_restricted_marker,
                    fee_refund.amount.u128(),
                    fee_refund.denom.to_owned(),
                    bid_order.owner.to_owned(),
                    env.contract.address.to_owned(),
                    env.contract.address,
                );
            }
        }

        bid_order.update_remaining_amounts(&Action::Fill {
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
        })?;

        bid_order.update_remaining_amounts(&Action::Refund {
            fee: bid_fee_refund,
            quote: Coin {
                denom: bid_order.quote.denom.to_owned(),
                amount: Uint128::new(bid_quote_refund),
            },
        })?;
    } else {
        bid_order.update_remaining_amounts(&Action::Fill {
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
        })?;
    }

    // finally update or remove the orders from storage
    if ask_order.size.is_zero() {
        ASKS_V1.remove(deps.storage, ask_id.as_bytes());
    } else {
        ASKS_V1.update(deps.storage, ask_id.as_bytes(), |_| -> StdResult<_> {
            Ok(ask_order)
        })?;
    }

    if bid_order.get_remaining_base().eq(&Uint128::zero()) {
        BIDS_V3.remove(deps.storage, bid_id.as_bytes());
    } else {
        BIDS_V3.update(deps.storage, bid_id.as_bytes(), |_| -> StdResult<_> {
            Ok(bid_order)
        })?;
    }

    Ok(response)
}

// smart contract migrate/upgrade entrypoint
#[entry_point]
pub fn migrate(mut deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    msg.validate()?;

    // build response
    let mut response: Response = Response::new();

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
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    msg.validate()?;

    match msg {
        QueryMsg::GetAsk { id } => {
            return to_binary(&ASKS_V1.load(deps.storage, id.as_bytes())?);
        }
        QueryMsg::GetBid { id } => {
            return to_binary(&BIDS_V3.load(deps.storage, id.as_bytes())?);
        }
        QueryMsg::GetContractInfo {} => to_binary(&get_contract_info(deps.storage)?),
        QueryMsg::GetVersionInfo {} => to_binary(&get_version_info(deps.storage)?),
    }
}

// unit tests
#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::{Addr, Storage, Uint128};

    use super::*;
    use provwasm_mocks::mock_provenance_dependencies;

    #[test]
    fn query_contract_info() {
        // setup
        let mut deps = mock_provenance_dependencies();
        setup_test_base(
            &mut deps.storage,
            &ContractInfoV3 {
                name: "contract_name".into(),
                bind_name: "".into(),
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
}
