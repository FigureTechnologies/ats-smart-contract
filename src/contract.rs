use cosmwasm_std::{
    attr, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128,
};
use provwasm_std::{bind_name, NameBinding, ProvenanceMsg, ProvenanceQuerier};

use crate::contract_info::{get_contract_info, set_contract_info, ContractInfo};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, Validate};
use crate::state::{
    get_ask_storage, get_ask_storage_read, get_bid_storage, get_bid_storage_read, AskOrder,
    AskOrderClass, AskOrderStatus, BidOrder,
};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::{Mul, Sub};

pub const CONTRACT_DEFINITION: &str = env!("CARGO_CRATE_NAME");
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// smart contract initialization entrypoint
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    msg.validate()?;

    // set contract info
    let contract_info = ContractInfo {
        name: msg.name,
        definition: CONTRACT_DEFINITION.into(),
        version: CONTRACT_VERSION.into(),
        bind_name: msg.bind_name,
        base_denom: msg.base_denom,
        convertible_base_denoms: msg.convertible_base_denoms,
        supported_quote_denoms: msg.supported_quote_denoms,
        executors: msg.executors,
        issuers: msg.issuers,
        ask_required_attributes: msg.ask_required_attributes,
        bid_required_attributes: msg.bid_required_attributes,
    };

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
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // validate execute message
    msg.validate()?;

    match msg {
        ExecuteMsg::CreateAsk { id, quote, price } => create_ask(
            deps,
            &info,
            AskOrder {
                base: info
                    .funds
                    .get(0)
                    .ok_or(ContractError::BaseQuantity)?
                    .to_owned(),
                class: AskOrderClass::Basic,
                id,
                owner: info.sender.to_owned(),
                quote,
                price,
                size: info.funds.get(0).ok_or(ContractError::BaseQuantity)?.amount,
            },
        ),
        ExecuteMsg::CreateBid {
            id,
            base,
            price,
            size,
        } => create_bid(
            deps,
            &info,
            BidOrder {
                base,
                id,
                owner: info.sender.to_owned(),
                price,
                quote: info
                    .funds
                    .get(0)
                    .ok_or(ContractError::QuoteQuantity)?
                    .to_owned(),
                size,
            },
        ),
        ExecuteMsg::CancelAsk { id } => cancel_ask(deps, info, id),
        ExecuteMsg::CancelBid { id } => cancel_bid(deps, info, id),
        ExecuteMsg::ExecuteMatch {
            ask_id,
            bid_id,
            price,
        } => execute_match(deps, env, info, ask_id, bid_id, price),
        ExecuteMsg::ApproveAsk { id } => approve_ask(deps, info, id),
        _ => Err(ContractError::Unauthorized {}),
    }
}

// create ask entrypoint
fn create_ask(
    deps: DepsMut,
    info: &MessageInfo,
    mut ask_order: AskOrder,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // error if order base is empty
    if ask_order.base.denom.is_empty() || ask_order.base.amount.is_zero() {
        return Err(ContractError::BaseQuantity);
    }

    let contract_info = get_contract_info(deps.storage)?;

    // error if order base is not contract base nor contract convertible base
    if ask_order.base.denom.ne(&contract_info.base_denom)
        && !contract_info
            .convertible_base_denoms
            .contains(&ask_order.base.denom)
    {
        return Err(ContractError::InconvertibleBaseDenom);
    }

    // error if quote denom unsupported
    if !contract_info
        .supported_quote_denoms
        .contains(&ask_order.quote)
    {
        return Err(ContractError::UnsupportedQuoteDenom);
    }

    // error if asker does not have required account attributes
    if !contract_info.ask_required_attributes.is_empty() {
        let querier = ProvenanceQuerier::new(&deps.querier);
        let none: Option<String> = None;
        let attributes_container = querier.get_attributes(&info.sender, none)?;
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

    if ask_order.base.denom.ne(&contract_info.base_denom) {
        ask_order.class = AskOrderClass::Convertible {
            status: AskOrderStatus::PendingIssuerApproval,
        };
    };

    ask_storage.save(&ask_order.id.as_bytes(), &ask_order)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![attr("action", "create_ask"), attr("id", &ask_order.id)],
        data: Some(to_binary(&ask_order)?),
    })
}

// create bid entrypoint
fn create_bid(
    deps: DepsMut,
    info: &MessageInfo,
    bid_order: BidOrder,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    // error if order quote is empty
    if bid_order.quote.denom.is_empty() || bid_order.quote.amount.is_zero() {
        return Err(ContractError::QuoteQuantity);
    }

    // error if quote does not match order
    if bid_order
        .quote
        .amount
        .ne(&Uint128(bid_order.size.u128() * bid_order.price.u128()))
    {
        return Err(ContractError::SentFundsOrderMismatch);
    }

    let contract_info = get_contract_info(deps.storage)?;

    // error if order quote is not supported quote denom
    if !&contract_info
        .supported_quote_denoms
        .contains(&bid_order.quote.denom)
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
        let attributes_container = querier.get_attributes(&info.sender, none)?;
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

    let mut bid_storage = get_bid_storage(deps.storage);

    bid_storage.save(&bid_order.id.as_bytes(), &bid_order)?;

    Ok(Response {
        submessages: vec![],
        messages: vec![],
        attributes: vec![attr("action", "create_bid"), attr("id", &bid_order.id)],
        data: Some(to_binary(&bid_order)?),
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
    let AskOrder {
        id, owner, base, ..
    } = ask_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::OrderLoad { error })?;
    if !info.sender.eq(&owner) {
        return Err(ContractError::Unauthorized);
    }

    // remove the ask order from storage
    let mut ask_storage = get_ask_storage(deps.storage);
    ask_storage.remove(id.as_bytes());

    // 'send base back to owner' message
    let response = Response {
        submessages: vec![],
        messages: vec![BankMsg::Send {
            to_address: owner,
            amount: vec![base],
        }
        .into()],
        attributes: vec![attr("action", "cancel_ask"), attr("id", id)],
        data: None,
    };

    Ok(response)
}

// cancel bid entrypoint
fn cancel_bid(
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

    let bid_storage = get_bid_storage_read(deps.storage);
    let BidOrder {
        id, owner, quote, ..
    } = bid_storage
        .load(id.as_bytes())
        .map_err(|error| ContractError::OrderLoad { error })?;
    if !info.sender.eq(&owner) {
        return Err(ContractError::Unauthorized);
    }

    // remove the ask order from storage
    let mut bid_storage = get_bid_storage(deps.storage);
    bid_storage.remove(id.as_bytes());

    // 'send quote back to owner' message
    let response = Response {
        submessages: vec![],
        messages: vec![BankMsg::Send {
            to_address: owner,
            amount: vec![quote],
        }
        .into()],
        attributes: vec![attr("action", "cancel_bid"), attr("id", id)],
        data: None,
    };

    Ok(response)
}

// approve an ask order by sending base funds in exchange for convertible base
fn approve_ask(
    _deps: DepsMut,
    _info: MessageInfo,
    _ask_id: String,
) -> Result<Response<ProvenanceMsg>, ContractError> {
    Err(ContractError::Unauthorized)
}

// match and execute an ask and bid order
fn execute_match(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    ask_id: String,
    bid_id: String,
    price: Uint128,
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
        .map_err(|error| ContractError::OrderLoad { error })?;

    let bid_storage_read = get_bid_storage_read(deps.storage);
    let mut bid_order = bid_storage_read
        .load(bid_id.as_bytes())
        .map_err(|error| ContractError::OrderLoad { error })?;

    match ask_order.price.cmp(&bid_order.price) {
        // order prices overlap, use ask or bid price determined by execute msg provided price
        Ordering::Less => {
            if price.ne(&ask_order.price) && price.ne(&bid_order.price) {
                return Err(ContractError::AskBidPriceMismatch);
            }
        }
        // if order prices are equal, execute price should be equal
        Ordering::Equal => {
            if price.ne(&ask_order.price) {
                return Err(ContractError::AskBidPriceMismatch);
            }
        }
        // ask price is greater than bid price, normal price spread behavior and should not match
        Ordering::Greater => {
            return Err(ContractError::AskBidPriceMismatch);
        }
    }

    //  branch on AskOrder type:
    //  - Basic: bilateral txn
    //  - Convertible: trilateral txn
    let response = match ask_order.class {
        AskOrderClass::Basic => {
            let base_size_to_send: Uint128;

            // at least one side of the order will always execute fully, both sides if order sizes equal
            // use the lesser of ask_order.size or bid_order.size.
            // else clause handles both bid_order.size less or equals cases
            if ask_order.size < bid_order.size {
                base_size_to_send = ask_order.size;
            } else {
                base_size_to_send = bid_order.size;
            }

            // calculate quote total. Uint128.mul only accepts Decimal types so need to (un)wrap
            let quote_total = Uint128(price.u128().mul(base_size_to_send.u128()));

            ask_order.size = ask_order.size.sub(base_size_to_send)?;
            ask_order.base.amount = ask_order.base.amount.sub(base_size_to_send)?;
            bid_order.size = bid_order.size.sub(base_size_to_send)?;
            bid_order.quote.amount = bid_order.quote.amount.sub(quote_total)?;

            // calculate refund to bidder if bid order is completed but quote funds remain
            let mut bidder_refund = Uint128(0);
            if bid_order.size.is_zero() && !bid_order.quote.amount.is_zero() {
                bidder_refund = bid_order.quote.amount;
                bid_order.quote.amount = bid_order.quote.amount.sub(bidder_refund)?;
            }

            // 'send quote to asker' and 'send base to bidder' messages
            Response {
                submessages: vec![],
                messages: vec![
                    BankMsg::Send {
                        to_address: ask_order.owner.clone(),
                        amount: vec![Coin {
                            denom: ask_order.quote.clone(),
                            amount: quote_total,
                        }],
                    }
                    .into(),
                    BankMsg::Send {
                        to_address: bid_order.owner.clone(),
                        amount:
                        // bid order completed, refund any remaining quote funds to bidder
                        if bidder_refund.is_zero() {
                            vec![Coin {
                                denom: bid_order.base.clone(),
                                amount: base_size_to_send,
                            }]
                        } else {
                            vec![
                                Coin {
                                    denom: bid_order.base.clone(),
                                    amount: base_size_to_send,
                                },
                                Coin {
                                    denom: bid_order.quote.denom.clone(),
                                    amount: bidder_refund,
                                },
                            ]
                        },
                    }
                    .into(),
                ],
                attributes: vec![
                    attr("action", "execute"),
                    attr("ask_id", &ask_id),
                    attr("bid_id", &bid_id),
                ],
                data: None,
            }
        }
        AskOrderClass::Convertible { status } => {
            if status.ne(&AskOrderStatus::Ready) {
                return Err(ContractError::AskOrderNotReady {
                    current_status: format!("{:?}", status),
                });
            };
            return Err(ContractError::Std(StdError::generic_err(
                "Unsupported Action",
            )));
        }
    };

    // finally update or remove the orders from storage
    if ask_order.size.is_zero() && ask_order.base.amount.is_zero() {
        get_ask_storage(deps.storage).remove(&ask_id.as_bytes());
    } else {
        get_ask_storage(deps.storage)
            .update(&ask_id.as_bytes(), |_| -> StdResult<_> { Ok(ask_order) })?;
    }

    if bid_order.size.is_zero() && bid_order.quote.amount.is_zero() {
        get_bid_storage(deps.storage).remove(&bid_id.as_bytes());
    } else {
        get_bid_storage(deps.storage)
            .update(&bid_id.as_bytes(), |_| -> StdResult<_> { Ok(bid_order) })?;
    }

    Ok(response)
}

// smart contract query entrypoint
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
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
    }
}

// unit tests
#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_env, mock_info, MOCK_CONTRACT_ADDR};
    use cosmwasm_std::CosmosMsg;
    use cosmwasm_std::{coin, coins, BankMsg, HumanAddr, Storage, Uint128};
    use provwasm_std::{NameMsgParams, ProvenanceMsg, ProvenanceMsgParams, ProvenanceRoute};

    use crate::contract_info::ContractInfo;
    use crate::state::{get_bid_storage_read, AskOrderClass};

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
            executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
            issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
                            address: MOCK_CONTRACT_ADDR.into(),
                            restrict: true
                        }),
                        version: "2.0.0".to_string(),
                    })
                );
                let expected_contract_info = ContractInfo {
                    name: "contract_name".into(),
                    definition: CONTRACT_DEFINITION.to_string(),
                    version: CONTRACT_VERSION.to_string(),
                    bind_name: "contract_bind_name".into(),
                    base_denom: "base_denom".into(),
                    convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                    supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                    executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                    issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                    ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                    bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            executors: vec![],
            issuers: vec![],
            ask_required_attributes: vec![],
            bid_required_attributes: vec![],
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
    fn create_ask_valid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".into(),
                version: "ver".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            id: "ask_id".into(),
            price: Uint128(2),
            quote: "quote_1".into(),
        };

        let asker_info = mock_info("asker", &coins(2, "base_1"));

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
                assert_eq!(response.attributes.len(), 2);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(response.attributes[1], attr("id", "ask_id"));
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify ask order stored
        let ask_storage = get_ask_storage_read(&deps.storage);
        if let ExecuteMsg::CreateAsk { id, price, quote } = create_ask_msg {
            match ask_storage.load("ask_id".to_string().as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrder {
                            base: coin(2, "base_1"),
                            class: AskOrderClass::Basic,
                            id,
                            owner: "asker".into(),
                            price,
                            quote,
                            size: Uint128(2)
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_1".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            id: "ask_id".into(),
            price: Uint128(2),
            quote: "quote_1".into(),
        };

        let asker_info = mock_info("asker", &coins(2, "base_1"));

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
                assert_eq!(response.attributes.len(), 2);
                assert_eq!(response.attributes[0], attr("action", "create_ask"));
                assert_eq!(response.attributes[1], attr("id", "ask_id"));
            }
            Err(error) => {
                panic!("failed to create ask: {:?}", error)
            }
        }

        // verify ask order stored
        let ask_storage = get_ask_storage_read(&deps.storage);
        if let ExecuteMsg::CreateAsk { id, quote, price } = create_ask_msg {
            match ask_storage.load("ask_id".to_string().as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        AskOrder {
                            base: coin(2, "base_1"),
                            class: AskOrderClass::Basic,
                            id,
                            owner: asker_info.sender,
                            price,
                            quote,
                            size: Uint128(2),
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
    fn create_ask_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create ask missing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "".into(),
            price: Uint128(0),
            quote: "".into(),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(100, "base_1")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::InvalidFields { fields } => {
                    assert!(fields.contains(&"id".into()));
                    assert!(fields.contains(&"price".into()));
                    assert!(fields.contains(&"quote".into()));
                }
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_missing_base() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create ask missing id
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ask_id".into(),
            price: Uint128(2),
            quote: "quote_1".into(),
        };

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("asker", &coins(0, "")),
            create_ask_msg,
        );

        // verify execute create ask response
        match create_ask_response {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::BaseQuantity => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_ask_inconvertible_base() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create ask with inconvertible base
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "id".into(),
            price: Uint128(2),
            quote: "quote_1".into(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create ask with unsupported quote
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "id".into(),
            price: Uint128(2),
            quote: "unsupported".into(),
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
    fn create_ask_wrong_account_attributes() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create ask data
        let create_ask_msg = ExecuteMsg::CreateAsk {
            id: "ask_id".into(),
            price: Uint128(2),
            quote: "quote_1".into(),
        };

        let asker_info = mock_info("asker", &coins(2, "base_denom"));

        // execute create ask
        let create_ask_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            create_ask_msg.clone(),
        );

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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            id: "bid_id".into(),
            price: Uint128(2),
            base: "base_denom".into(),
            size: Uint128(100),
        };

        let bidder_info = mock_info("bidder", &coins(200, "quote_1"));

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
                assert_eq!(response.attributes.len(), 2);
                assert_eq!(response.attributes[0], attr("action", "create_bid"));
                assert_eq!(response.attributes[1], attr("id", "bid_id"));
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
            price,
            size,
        } = create_bid_msg
        {
            match bid_storage.load("bid_id".to_string().as_bytes()) {
                Ok(stored_order) => {
                    assert_eq!(
                        stored_order,
                        BidOrder {
                            base,
                            id,
                            owner: bidder_info.sender,
                            price,
                            quote: coin(200, "quote_1"),
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
    fn create_bid_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid missing id
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "".into(),
            base: "".into(),
            price: Uint128(0),
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
    fn create_bid_missing_quote() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid missing quote
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "bid_id".into(),
            base: "base_1".into(),
            price: Uint128(2),
            size: Uint128(100),
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
                ContractError::QuoteQuantity => {}
                error => panic!("unexpected error: {:?}", error),
            },
        }
    }

    #[test]
    fn create_bid_invalid_base() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid with invalid base
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "bid_id".into(),
            base: "notbasedenom".into(),
            price: Uint128(2),
            size: Uint128(10),
        };

        // execute create ask
        let create_bid_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("bidder", &coins(20, "quote_2")),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "bid_id".into(),
            base: "base_denom".into(),
            price: Uint128(2),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "bid_id".into(),
            base: "base_denom".into(),
            price: Uint128(2),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid data
        let create_bid_msg = ExecuteMsg::CreateBid {
            id: "bid_id".into(),
            base: "base_denom".into(),
            price: Uint128(2),
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
    fn cancel_ask_valid() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // cancel ask order
        let asker_info = mock_info("asker", &[]);

        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ask_id".to_string(),
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
                assert_eq!(cancel_ask_response.attributes[1], attr("id", "ask_id"));
                assert_eq!(cancel_ask_response.messages.len(), 1);
                assert_eq!(
                    cancel_ask_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: asker_info.sender,
                        amount: coins(100, "base_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage.load("ask_id".to_string().as_bytes()).is_err(),
            true
        );
    }

    #[test]
    fn cancel_ask_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        let asker_info = mock_info("asker", &[]);

        // cancel non-existent ask order returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "unknown_id".to_string(),
        };

        let cancel_response = execute(
            deps.as_mut(),
            mock_env(),
            asker_info.clone(),
            cancel_ask_msg,
        );

        match cancel_response {
            Err(error) => match error {
                ContractError::OrderLoad { .. } => {}
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        let asker_info = mock_info("asker", &[]);

        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(200, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: "not_asker".into(),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // cancel ask order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ask_id".to_string(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // cancel ask order with sent_funds returns ContractError::CancelWithFunds
        let asker_info = mock_info("asker", &coins(1, "sent_coin"));
        let cancel_ask_msg = ExecuteMsg::CancelAsk {
            id: "ask_id".to_string(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // create bid data
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(200, "quote_1"),
                size: Uint128(100),
            },
        );

        // cancel bid order
        let bidder_info = mock_info("bidder", &[]);

        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "bid_id".to_string(),
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
                assert_eq!(cancel_bid_response.attributes[1], attr("id", "bid_id"));
                assert_eq!(cancel_bid_response.messages.len(), 1);
                assert_eq!(
                    cancel_bid_response.messages[0],
                    CosmosMsg::Bank(BankMsg::Send {
                        to_address: bidder_info.sender,
                        amount: coins(200, "quote_1"),
                    })
                );
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage.load("bid_id".to_string().as_bytes()).is_err(),
            true
        );
    }

    #[test]
    fn cancel_bid_invalid_data() {
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        let bidder_info = mock_info("bidder", &[]);

        // cancel non-existent bid order returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "unknown_id".to_string(),
        };

        let cancel_response = execute(
            deps.as_mut(),
            mock_env(),
            bidder_info.clone(),
            cancel_bid_msg,
        );

        match cancel_response {
            Err(error) => match error {
                ContractError::OrderLoad { .. } => {}
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        let bidder_info = mock_info("bidder", &[]);

        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: "not_bidder".into(),
                price: Uint128(2),
                quote: coin(100, "quote_1"),
                size: Uint128(200),
            },
        );

        // cancel bid order with sender not equal to owner returns ContractError::Unauthorized
        let cancel_bid_msg = ExecuteMsg::CancelBid {
            id: "bid_id".to_string(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // cancel bid order with sent_funds returns ContractError::CancelWithFunds
        let bidder_info = mock_info("bidder", &coins(1, "sent_coin"));
        let cancel_bid_msg = ExecuteMsg::CancelAsk {
            id: "bid_id".to_string(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(200, "quote_1"),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
                assert_eq!(execute_response.attributes.len(), 3);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(execute_response.attributes[1], attr("ask_id", "ask_id"));
                assert_eq!(execute_response.attributes[2], attr("bid_id", "bid_id"));
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
            ask_storage.load("ask_id".to_string().as_bytes()).is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage.load("bid_id".to_string().as_bytes()).is_err(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(30, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(30),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(20, "quote_1"),
                size: Uint128(10),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
                assert_eq!(execute_response.attributes.len(), 3);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(execute_response.attributes[1], attr("ask_id", "ask_id"));
                assert_eq!(execute_response.attributes[2], attr("bid_id", "bid_id"));
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
        match ask_storage.load("ask_id".to_string().as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    AskOrder {
                        base: coin(20, "base_1"),
                        class: AskOrderClass::Basic,
                        id: "ask_id".into(),
                        owner: "asker".into(),
                        price: Uint128(2),
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
            bid_storage.load("bid_id".to_string().as_bytes()).is_err(),
            true
        );
    }

    #[test]
    fn execute_partial_bid_order() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(50, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(50),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(200, "quote_1"),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
                assert_eq!(execute_response.attributes.len(), 3);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(execute_response.attributes[1], attr("ask_id", "ask_id"));
                assert_eq!(execute_response.attributes[2], attr("bid_id", "bid_id"));
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
        match bid_storage.load("bid_id".to_string().as_bytes()) {
            Ok(stored_order) => {
                assert_eq!(
                    stored_order,
                    BidOrder {
                        base: "base_1".into(),
                        id: "bid_id".into(),
                        owner: "bidder".into(),
                        price: Uint128(2),
                        quote: coin(100, "quote_1"),
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
        assert_eq!(
            ask_storage.load("ask_id".to_string().as_bytes()).is_err(),
            true
        );
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(4),
                quote: coin(400, "quote_1"),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
                assert_eq!(execute_response.attributes.len(), 3);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(execute_response.attributes[1], attr("ask_id", "ask_id"));
                assert_eq!(execute_response.attributes[2], attr("bid_id", "bid_id"));
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
                        amount: vec![coin(100, "base_1"), coin(200, "quote_1")],
                    })
                );
            }
        }

        // verify ask order removed from storage
        let ask_storage = get_ask_storage_read(&deps.storage);
        assert_eq!(
            ask_storage.load("ask_id".to_string().as_bytes()).is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage.load("bid_id".to_string().as_bytes()).is_err(),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(100),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(4),
                quote: coin(400, "quote_1"),
                size: Uint128(100),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(4),
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
                assert_eq!(execute_response.attributes.len(), 3);
                assert_eq!(execute_response.attributes[0], attr("action", "execute"));
                assert_eq!(execute_response.attributes[1], attr("ask_id", "ask_id"));
                assert_eq!(execute_response.attributes[2], attr("bid_id", "bid_id"));
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
            ask_storage.load("ask_id".to_string().as_bytes()).is_err(),
            true
        );

        // verify bid order removed from storage
        let bid_storage = get_bid_storage_read(&deps.storage);
        assert_eq!(
            bid_storage.load("bid_id".to_string().as_bytes()).is_err(),
            true
        );
    }

    #[test]
    fn execute_invalid_data() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "".into(),
            bid_id: "".into(),
            price: Uint128(0),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // execute by non-executor
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );
        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(200, "base_1"),
                class: AskOrderClass::Convertible {
                    status: AskOrderStatus::PendingIssuerApproval,
                },
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(100, "quote_1"),
                size: Uint128(200),
            },
        );

        // execute when ask order not ready returns ContractError::PendingIssuerApproval
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(100, "quote_1"),
                size: Uint128(200),
            },
        );

        // execute on non-existent ask order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "no_ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::OrderLoad { .. }) => {}
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );
        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(200, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(2),
                quote: "quote_1".into(),
                size: Uint128(200),
            },
        );

        // execute on non-existent bid order and bid order returns ContractError::OrderLoad
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "no_bid_id".into(),
            price: Uint128(2),
        };

        let execute_response = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("exec_1", &[]),
            execute_msg,
        );

        match execute_response {
            Err(ContractError::OrderLoad { .. }) => {}
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                executors: vec![
                    HumanAddr::from(
                        "exec_1\
                ",
                    ),
                    HumanAddr::from("exec_2"),
                ],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                supported_quote_denoms: vec![],
            },
        );

        // execute with sent_funds returns ContractError::ExecuteWithFunds
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        store_test_ask(
            &mut deps.storage,
            &AskOrder {
                base: coin(100, "base_1"),
                class: AskOrderClass::Basic,
                id: "ask_id".into(),
                owner: HumanAddr("asker".into()),
                price: Uint128(3),
                quote: "quote_1".into(),
                size: Uint128(300),
            },
        );

        // store valid bid order
        store_test_bid(
            &mut deps.storage,
            &BidOrder {
                base: "base_1".into(),
                id: "bid_id".into(),
                owner: HumanAddr("bidder".into()),
                price: Uint128(2),
                quote: coin(100, "quote_1"),
                size: Uint128(200),
            },
        );

        // execute on matched ask order and bid order
        let execute_msg = ExecuteMsg::ExecuteMatch {
            ask_id: "ask_id".into(),
            bid_id: "bid_id".into(),
            price: Uint128(2),
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
    fn query_with_valid_data() {
        // setup
        let mut deps = mock_dependencies(&[]);
        setup_test_base(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            },
        );

        // store valid ask order
        let ask_order = AskOrder {
            base: coin(200, "base_1"),
            class: AskOrderClass::Basic,
            id: "ask_id".into(),
            owner: HumanAddr("asker".into()),
            price: Uint128(2),
            quote: "quote_1".into(),
            size: Uint128(200),
        };

        let mut ask_storage = get_ask_storage(&mut deps.storage);
        if let Err(error) = ask_storage.save(&"ask_id".as_bytes(), &ask_order) {
            panic!("unexpected error: {:?}", error)
        };

        // store valid bid order
        let bid_order = BidOrder {
            base: "base_1".into(),
            id: "bid_id".into(),
            owner: HumanAddr("bidder".into()),
            price: Uint128(2),
            quote: coin(100, "quote_1"),
            size: Uint128(100),
        };

        let mut bid_storage = get_bid_storage(&mut deps.storage);
        if let Err(error) = bid_storage.save(&bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };

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

        // query for ask order
        let query_ask_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetAsk {
                id: "ask_id".into(),
            },
        );

        assert_eq!(query_ask_response, to_binary(&ask_order));

        // query for bid order
        let query_bid_response = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBid {
                id: bid_order.id.clone(),
            },
        );

        assert_eq!(query_bid_response, to_binary(&bid_order));
    }

    fn setup_test_base(storage: &mut dyn Storage, contract_info: &ContractInfo) {
        if let Err(error) = set_contract_info(storage, contract_info) {
            panic!("unexpected error: {:?}", error)
        }
    }

    fn store_test_ask(storage: &mut dyn Storage, ask_order: &AskOrder) {
        let mut ask_storage = get_ask_storage(storage);
        if let Err(error) = ask_storage.save(&ask_order.id.as_bytes(), &ask_order) {
            panic!("unexpected error: {:?}", error)
        };
    }

    fn store_test_bid(storage: &mut dyn Storage, bid_order: &BidOrder) {
        let mut bid_storage = get_bid_storage(storage);
        if let Err(error) = bid_storage.save(&bid_order.id.as_bytes(), &bid_order) {
            panic!("unexpected error: {:?}", error);
        };
    }
}
