use cosm_orc::orchestrator::Denom;
use lazy_static::lazy_static;
use std::env;
use std::str::FromStr;

pub(crate) const PREFIX: &str = "tp";
pub(crate) const DEFAULT_CHAIN_ID: &str = "chain-local";
pub(crate) const DERIVATION_PATH: &str = "m/44'/1'/0'/0/0'";
pub(crate) const CONTRACT_NAME: &str = "ats_smart_contract";
pub(crate) const DEFAULT_GRPC_ENDPOINT: &str = "http://localhost:9090/";
pub(crate) const DEFAULT_GAS_PRICE: f64 = 1905.0;
pub(crate) const DEFAULT_GAS_ADJUSTMENT: f64 = 1.5;

lazy_static! {
    pub(crate) static ref PIO_HOME: String = env::var("PIO_HOME").expect("Missing PIO_HOME");
    pub(crate) static ref WASM_DIR: String = PIO_HOME.clone();
    pub(crate) static ref HASH_DENOM: Denom = Denom::from_str("nhash").unwrap();
    pub(crate) static ref BASE_DENOM: Denom = Denom::from_str("gme.local").unwrap();
    pub(crate) static ref QUOTE_DENOM: Denom = Denom::from_str("usd.local").unwrap();
}
