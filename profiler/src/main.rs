use clap::{Arg, ArgAction, Command};
use cosm_orc::config::cfg::Config as CosmConfig;
use cosm_orc::config::ChainConfig;
use cosm_orc::cosm_tome::clients::client::CosmosClient;
use cosm_orc::cosm_tome::modules::cosmwasm::model::StoreCodeResponse;
use cosm_orc::orchestrator::cosm_orc::CosmOrc;
use cosm_orc::orchestrator::deploy::DeployInfo;
use cosm_orc::orchestrator::CosmosgRPC;
use cosm_orc::orchestrator::{
    Address, Coin, ExecResponse, InstantiateResponse, Key, QueryResponse, SigningKey,
};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Serialize;
use std::collections::HashMap;
use std::process::exit;
use std::str::FromStr;
use uuid::Uuid;

mod constants;
mod error;
mod execute;
mod extension;
mod instantiate;
mod provenanced;
mod query;

use constants::*;
use error::{ProfilerError, Result};
use extension::{CosmResponseExt, SerializeExt};

macro_rules! log {
    ($($arg:tt)*) => { eprintln!($($arg)*) }
}

#[derive(Serialize)]
struct ContractResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    code_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
}

#[derive(Clone, Debug)]
struct Config {
    contract_name: String,
    wasm_dir: Option<String>,
    cosm_config: CosmConfig,
    profiling: bool,
    verbose: bool,
}

impl From<Config> for CosmConfig {
    fn from(value: Config) -> Self {
        value.cosm_config
    }
}

impl Config {
    pub(crate) fn new() -> Self {
        Config {
            contract_name: CONTRACT_NAME.to_owned(),
            wasm_dir: None,
            cosm_config: CosmConfig {
                chain_cfg: ChainConfig {
                    denom: HASH_DENOM.to_string(),
                    prefix: PREFIX.to_owned(),
                    chain_id: DEFAULT_CHAIN_ID.to_owned(),
                    derivation_path: DERIVATION_PATH.to_owned(),
                    rpc_endpoint: None,
                    grpc_endpoint: Some(DEFAULT_GRPC_ENDPOINT.to_owned()),
                    gas_price: DEFAULT_GAS_PRICE,
                    gas_adjustment: DEFAULT_GAS_ADJUSTMENT,
                },
                contract_deploy_info: HashMap::new(),
            },
            profiling: true,
            verbose: false,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_profiling(&mut self, enable: bool) {
        self.profiling = enable;
    }

    pub(crate) fn set_verbose(&mut self, enable: bool) {
        self.verbose = enable;
    }

    pub(crate) fn set_wasm_dir<S: Into<String>>(&mut self, wasm_dir: S) {
        self.wasm_dir = Some(wasm_dir.into());
    }

    /// Set the code ID of the smart contract to use
    pub(crate) fn with_deploy_info(&mut self, code_id: Option<u64>, address: Option<String>) {
        let deploy_info = DeployInfo { code_id, address };
        self.cosm_config
            .contract_deploy_info
            .insert(self.contract_name.clone(), deploy_info);
    }

    pub(crate) fn new_cosm(&self) -> Result<CosmOrc<CosmosgRPC>> {
        Ok(CosmOrc::new(self.cosm_config.clone(), self.profiling)
            .map_err(Into::<ProfilerError>::into)?)
    }
}

fn rand_string(n: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
struct OrderbookName(String);

impl OrderbookName {
    fn new_random() -> Self {
        let suffix = rand_string(8);
        let name = format!("orderbook-{}.sc.pb", suffix);
        Self(name)
    }
}

impl From<OrderbookName> for String {
    fn from(value: OrderbookName) -> Self {
        value.0
    }
}

impl From<&str> for OrderbookName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for OrderbookName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Bootstraps a new `CosmOrc<_>` instance given a `Config`.
///
/// This function will:
///
/// (1) check if a contract exists according to `config.contract_name`.
/// (2) if not, store the contract, fetch the resulting code ID, and
///   update `config` with the value
/// (3) instantiate a new `CosmOrc<_>` client with the full contract info,
///   including code ID, and optionally, the address of a previously
///   instantiated contract.
///
fn bootstrap_cosm(
    signing_key: &SigningKey,
    config: &mut Config,
    code_id: Option<u64>,
    address: Option<String>,
) -> Result<(CosmOrc<CosmosgRPC>, Option<u64>, Option<String>)> {
    // Get a code ID, either as an argument, or if it's in the config already:
    let use_code_id = match code_id {
        code_id @ Some(_) => code_id,
        None => config
            .cosm_config
            .contract_deploy_info
            .get(&config.contract_name)
            .and_then(|info| info.code_id),
    };

    let (code_id, address) = if let Some(code_id) = use_code_id {
        config.with_deploy_info(Some(code_id), address.clone());
        (Some(code_id), address)
    } else {
        // Update the config with the code ID returned from the result of
        // `store_contract()`:
        let mut cosm = config.new_cosm()?;

        let wasm_dir = config
            .wasm_dir
            .clone()
            .ok_or(ProfilerError::MissingWasmDirectory)?;

        // Store the actual contract since we assume it's not on-chain yet:
        let response: Vec<StoreCodeResponse> = cosm
            .store_contracts(&wasm_dir, signing_key, None)
            .map_err(Into::<ProfilerError>::into)?;

        if config.verbose {
            eprintln!("{:#?}", response);
        }

        let contract_response = response
            .first()
            .ok_or(ProfilerError::MissingStoreCodeResponse)?
            .to_owned();

        // Pull out the code ID from the store response and build a new cosm-orc
        // client using that:
        let code_id: u64 = contract_response.code_id;
        config.with_deploy_info(Some(code_id), None);
        (Some(code_id), None)
    };

    let new_cosm = config.new_cosm()?;

    Ok((new_cosm, code_id, address))
}

/// Gets the version information of the ATS smart contract.
fn get_contract_version<C: CosmosClient>(
    cosm: &CosmOrc<C>,
    config: &Config,
) -> Result<QueryResponse> {
    cosm.query(&config.contract_name, &query::get_contract_version())
        .map_err(Into::into)
}

/// Gets the version information of the ATS smart contract.
fn get_contract_info<C: CosmosClient>(cosm: &CosmOrc<C>, config: &Config) -> Result<QueryResponse> {
    cosm.query(&config.contract_name, &query::get_contract_info())
        .map_err(Into::into)
}

/// Instantiate a smart contract.
fn instantiate_contract<C: CosmosClient>(
    cosm: &mut CosmOrc<C>,
    signing_key: &SigningKey,
    config: &Config,
) -> Result<InstantiateResponse> {
    let node0_address = provenanced::get_node0_address()?;
    let orderbook_name = OrderbookName::new_random();

    let msg = instantiate::InstantiateMsg::build(&node0_address, orderbook_name);

    if config.verbose {
        log!("Executing JSON = {}", msg.json_string()?);
    }

    let admin_address = Address::from_str(&node0_address)?;
    // This must match the name of the .wasm file:, e.g. "ats_smart_contract.wasm"
    // See https://docs.rs/cosm-orc/4.0.1/cosm_orc/orchestrator/cosm_orc/struct.CosmOrc.html#method.store_contracts
    let admin = Some(admin_address);
    let funds: Vec<Coin> = vec![];

    cosm.instantiate(
        config.contract_name.as_str(),
        "instantiate contract",
        &msg,
        signing_key,
        admin,
        funds,
    )
    .map_err(Into::into)
}

/// Execute a BID order.
fn execute_bid<C: CosmosClient>(
    cosm: &mut CosmOrc<C>,
    signing_key: &SigningKey,
    config: &Config,
    price: &str,
    size: &str,
    quote_size: &str,
) -> Result<(Uuid, ExecResponse)> {
    let id = Uuid::new_v4();
    let msg = execute::create_bid(&id, &BASE_DENOM, price, size, &QUOTE_DENOM, quote_size);
    let funds: Vec<Coin> = vec![Coin {
        denom: (*QUOTE_DENOM).clone(),
        amount: quote_size.parse::<u128>()?,
    }];

    if config.verbose {
        log!("Executing JSON = {}", msg.json_string()?);
    }

    cosm.execute(
        CONTRACT_NAME,
        &format!(
            "bid {{ id = {id}, base={base}, quote={quote}, price={price}, quote_size={quote_size}, size={size} }}",
            id = id,
            base = *BASE_DENOM,
            quote = *QUOTE_DENOM,
            price = price,
            size = size
        ),
        &msg,
        signing_key,
        funds,
    )
    .map(|r| (id, r))
    .map_err(Into::into)
}

/// Execute an ASK order.
fn execute_ask<C: CosmosClient>(
    cosm: &mut CosmOrc<C>,
    signing_key: &SigningKey,
    config: &Config,
    price: &str,
    size: &str,
) -> Result<(Uuid, ExecResponse)> {
    let id = Uuid::new_v4();
    let msg = execute::create_ask(&id, &QUOTE_DENOM, price, &BASE_DENOM, size);
    let funds: Vec<Coin> = vec![Coin {
        denom: (*BASE_DENOM).clone(),
        amount: size.parse::<u128>()?,
    }];

    if config.verbose {
        log!("Executing JSON = {}", msg.json_string()?);
    }

    cosm.execute(
        CONTRACT_NAME,
        &format!(
            "ask {{ id={id}, base={base}, quote={quote}, price={price}, size={size} }}",
            id = id,
            base = *BASE_DENOM,
            quote = *QUOTE_DENOM,
            price = price,
            size = size
        ),
        &msg,
        signing_key,
        funds,
    )
    .map(|r| (id, r))
    .map_err(Into::into)
}

/// Execute an order match.
fn execute_match<C: CosmosClient>(
    cosm: &mut CosmOrc<C>,
    signing_key: &SigningKey,
    config: &Config,
    bid_id: &Uuid,
    ask_id: &Uuid,
    price: &str,
    size: &str,
) -> Result<ExecResponse> {
    let msg = execute::execute_match(bid_id, ask_id, price, size);

    if config.verbose {
        log!("Executing JSON = {}", msg.json_string()?);
    }

    let funds: Vec<Coin> = vec![];
    cosm.execute(
        CONTRACT_NAME,
        &format!(
            "match {{ bid_id={bid_id}, ask_id={ask_id}, price={price}, size={size} }}",
            bid_id = bid_id,
            ask_id = ask_id,
            price = price,
            size = size
        ),
        &msg,
        signing_key,
        funds,
    )
    .map_err(Into::into)
}

fn run() -> Result<()> {
    let mut config = Config::new();
    let app = Command::new(env!("CARGO_PKG_NAME"))
        .about("ATS Smart Contract profiler")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Enable verbose output"),
        )
        .arg(
            Arg::new("buyer-mnemonic")
                .long("buyer")
                .env("BUYER_MNEMONIC")
                .action(ArgAction::Set)
                .required(true)
                .help("Buyer BIP39 mnemonic phrase"),
        )
        .arg(
            Arg::new("seller-mnemonic")
                .long("seller")
                .env("SELLER_MNEMONIC")
                .action(ArgAction::Set)
                .required(true)
                .help("Seller BIP39 mnemonic phrase"),
        )
        .subcommand(
            Command::new("store")
                .about("Store a smart contract on chain")
                .arg(
                    Arg::new("wasm-dir")
                        .long("wasm-dir")
                        .required(true)
                        .help("WASM artifact directory"),
                ),
        )
        .subcommand(
            Command::new("instantiate")
                .about("Instantiate a previously stored smart contract")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                ),
        )
        .subcommand(
            Command::new("contract-version")
                .about("View contract version")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                )
                .arg(
                    Arg::new("address")
                        .long("address")
                        .required(true)
                        .help("The address of a previously instantiated smart contract"),
                ),
        )
        .subcommand(
            Command::new("contract-info")
                .about("View contract information")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                )
                .arg(
                    Arg::new("address")
                        .long("address")
                        .required(true)
                        .help("The address of a previously instantiated smart contract"),
                ),
        )
        .subcommand(
            Command::new("place-bid")
                .about("Place a BID order")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                )
                .arg(
                    Arg::new("address")
                        .long("address")
                        .required(true)
                        .help("The address of a previously instantiated smart contract"),
                )
                .arg(Arg::new("price").long("price").default_value("2"))
                .arg(Arg::new("size").long("size").default_value("500"))
                .arg(
                    Arg::new("quote-size")
                        .long("quote-size")
                        .default_value("1000"),
                ),
        )
        .subcommand(
            Command::new("place-ask")
                .about("Place an ASK order")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                )
                .arg(
                    Arg::new("address")
                        .long("address")
                        .required(true)
                        .help("The address of a previously instantiated smart contract"),
                )
                .arg(Arg::new("price").long("price").default_value("2"))
                .arg(Arg::new("size").long("size").default_value("500")),
        )
        .subcommand(
            Command::new("execute-match")
                .about("Execute an order match")
                .arg(
                    Arg::new("code-id")
                        .long("code-id")
                        .required(true)
                        .help("The code ID of an existing smart contract"),
                )
                .arg(
                    Arg::new("address")
                        .long("address")
                        .required(true)
                        .help("The address of a previously instantiated smart contract"),
                )
                .arg(Arg::new("bid-id").long("bid-id").required(true))
                .arg(Arg::new("ask-id").long("ask-id").required(true))
                .arg(Arg::new("price").long("price").default_value("2"))
                .arg(Arg::new("size").long("size").default_value("500")),
        );

    let matches = app.get_matches();

    if matches.contains_id("verbose") {
        config.set_verbose(true);
    }

    let buyer_mnemonic = matches
        .get_one::<String>("buyer-mnemonic")
        .cloned()
        .ok_or(ProfilerError::MissingMnemonic("buyer".to_owned()))?;
    let seller_mnemonic = matches
        .get_one::<String>("seller-mnemonic")
        .cloned()
        .ok_or(ProfilerError::MissingMnemonic("seller".to_owned()))?;

    let node0_key = SigningKey {
        name: "node0".to_string(),
        key: Key::Raw(provenanced::get_node0_private_key_bytes()?),
        derivation_path: DERIVATION_PATH.into(),
    };

    let buyer_key = SigningKey {
        name: "buyer".to_string(),
        key: Key::Mnemonic(buyer_mnemonic),
        derivation_path: DERIVATION_PATH.into(),
    };

    let seller_key = SigningKey {
        name: "seller".to_string(),
        key: Key::Mnemonic(seller_mnemonic),
        derivation_path: DERIVATION_PATH.into(),
    };

    match matches.subcommand() {
        Some(("store", sc_matches)) => {
            let wasm_dir = sc_matches
                .get_one::<String>("wasm-dir")
                .cloned()
                .ok_or(ProfilerError::MissingWasmDirectory)?;
            config.set_wasm_dir(wasm_dir);
            let (_, code_id, _) = bootstrap_cosm(&buyer_key, &mut config, None, None)?;
            println!(
                "{}",
                ContractResponse {
                    code_id,
                    address: None
                }
                .json_string()?
            );
        }
        Some(("instantiate", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let (mut cosm, code_id, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, None)?;
            let response = instantiate_contract(&mut cosm, &buyer_key, &config)?;

            if sc_matches.contains_id("verbose") {
                log!("response = {:#?}", response);
            }

            println!(
                "{}",
                ContractResponse {
                    code_id,
                    address: Some(response.address.to_string())
                }
                .json_string()?
            );
        }
        Some(("contract-version", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let address: Option<String> = sc_matches.get_one::<String>("address").cloned();

            let (cosm, _, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, address)?;
            let response = get_contract_version(&cosm, &config)?;

            println!("{}", response.to_utf8_string()?); // JSON
        }
        Some(("contract-info", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let address: Option<String> = sc_matches.get_one::<String>("address").cloned();

            let (cosm, _, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, address)?;
            let response = get_contract_info(&cosm, &config)?;

            println!("{}", response.to_utf8_string()?);
        }
        Some(("place-bid", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let address: Option<String> = sc_matches.get_one::<String>("address").cloned();
            let price = sc_matches.get_one::<String>("price").expect("price");
            let size = sc_matches.get_one::<String>("size").expect("size");
            let quote_size = sc_matches
                .get_one::<String>("quote-size")
                .expect("quote_size");

            let (mut cosm, _, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, address)?;
            let (bid_id, response) =
                execute_bid(&mut cosm, &buyer_key, &config, price, size, quote_size)?;

            if config.verbose {
                log!("response = {:#?}", response);
            }

            log!("Successfully submitted BID: ID = {}", bid_id.to_string());

            if config.profiling {
                let reports = cosm.gas_profiler_report().cloned().unwrap_or_default();
                let reports_json: String = serde_json::to_value(reports)?.to_string();

                log!("Profiler output:");
                println!("{}", reports_json);
            }
        }
        Some(("place-ask", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let address: Option<String> = sc_matches.get_one::<String>("address").cloned();
            let price = sc_matches.get_one::<String>("price").expect("price");
            let size = sc_matches.get_one::<String>("size").expect("size");

            let (mut cosm, _, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, address)?;
            let (ask_id, response) = execute_ask(&mut cosm, &seller_key, &config, price, size)?;

            log!("Successfully submitted ASK: ID = {}", ask_id.to_string());

            if config.verbose {
                log!("response = {:#?}", response);
            }

            if config.profiling {
                let reports = cosm.gas_profiler_report().cloned().unwrap_or_default();
                let reports_json: String = serde_json::to_value(reports)?.to_string();

                log!("Profiler output:");
                println!("{}", reports_json);
            }
        }
        Some(("execute-match", sc_matches)) => {
            let code_id = sc_matches
                .get_one::<String>("code-id")
                .and_then(|opt| opt.parse::<u64>().ok());
            let address: Option<String> = sc_matches.get_one::<String>("address").cloned();
            let bid_id = Uuid::parse_str(
                &sc_matches
                    .get_one::<String>("bid-id")
                    .cloned()
                    .expect("bid-id"),
            )?;
            let ask_id = Uuid::parse_str(
                &sc_matches
                    .get_one::<String>("ask-id")
                    .cloned()
                    .expect("ask-id"),
            )?;
            let price = sc_matches.get_one::<String>("price").expect("price");
            let size = sc_matches.get_one::<String>("size").expect("size");

            let (mut cosm, _, _) = bootstrap_cosm(&buyer_key, &mut config, code_id, address)?;
            let response = execute_match(
                &mut cosm, &node0_key, &config, &bid_id, &ask_id, price, size,
            )?;

            if config.verbose {
                log!("response = {:#?}", response);
            }

            if config.profiling {
                let reports = cosm.gas_profiler_report().cloned().unwrap_or_default();
                let reports_json: String = serde_json::to_value(reports)?.to_string();

                log!("Profiler output:");
                println!("{}", reports_json);
            }
        }
        Some((_, _)) => { }
        None => { }
    }

    Ok(())
}

fn main() {
    let code = match run() {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("{:#?}", e);
            1
        }
    };
    exit(code);
}
