use cosmwasm_std::{HumanAddr, StdResult, Storage};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;

const CONTRACT_INFO_NAMESPACE: &str = "contract_info";
pub const CONTRACT_INFO: Item<ContractInfo> = Item::new(CONTRACT_INFO_NAMESPACE);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfo {
    pub name: String,
    pub definition: String,
    pub version: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub executors: Vec<HumanAddr>,
    pub issuers: Vec<HumanAddr>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
}

pub fn set_contract_info(
    store: &mut dyn Storage,
    contract_info: &ContractInfo,
) -> Result<(), ContractError> {
    let result = CONTRACT_INFO.save(store, &contract_info);
    result.map_err(ContractError::Std)
}

pub fn get_contract_info(store: &dyn Storage) -> StdResult<ContractInfo> {
    CONTRACT_INFO.load(store)
}

#[cfg(test)]
mod tests {
    use provwasm_mocks::mock_dependencies;

    use crate::contract_info::{get_contract_info, set_contract_info, ContractInfo};
    use cosmwasm_std::HumanAddr;

    #[test]
    pub fn set_contract_info_with_valid_data() {
        let mut deps = mock_dependencies(&[]);
        let result = set_contract_info(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "def".to_string(),
                version: "ver".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quo_base_1".into(), "quo_base_2".into()],
                executors: vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")],
                issuers: vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            },
        );
        match result {
            Ok(()) => {}
            result => panic!("unexpected error: {:?}", result),
        }

        let contract_info = get_contract_info(&deps.storage);
        match contract_info {
            Ok(contract_info) => {
                assert_eq!(contract_info.name, "contract_name");
                assert_eq!(contract_info.definition, "def");
                assert_eq!(contract_info.version, "ver");
                assert_eq!(contract_info.bind_name, "contract_bind_name");
                assert_eq!(contract_info.base_denom, "base_denom");
                assert_eq!(
                    contract_info.convertible_base_denoms,
                    vec!["con_base_1", "con_base_2"]
                );
                assert_eq!(
                    contract_info.supported_quote_denoms,
                    vec!["quo_base_1", "quo_base_2"]
                );
                assert_eq!(
                    contract_info.executors,
                    vec![HumanAddr::from("exec_1"), HumanAddr::from("exec_2")]
                );
                assert_eq!(
                    contract_info.issuers,
                    vec![HumanAddr::from("issuer_1"), HumanAddr::from("issuer_2")]
                );
                assert_eq!(
                    contract_info.ask_required_attributes,
                    vec!["ask_tag_1", "ask_tag_2"]
                );
                assert_eq!(
                    contract_info.bid_required_attributes,
                    vec!["ask_tag_1", "ask_tag_2"]
                );
            }
            result => panic!("unexpected error: {:?}", result),
        }
    }
}
