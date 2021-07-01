use cosmwasm_std::{Addr, Api, Storage, Uint128};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use semver::{Version, VersionReq};

const CONTRACT_INFO_NAMESPACE: &str = "contract_info";

#[allow(deprecated)]
const CONTRACT_INFO: Item<ContractInfo> = Item::new(CONTRACT_INFO_NAMESPACE);
#[allow(deprecated)]
const CONTRACT_INFO_V1: Item<ContractInfoV1> = Item::new(CONTRACT_INFO_NAMESPACE);
const CONTRACT_INFO_V2: Item<ContractInfoV2> = Item::new(CONTRACT_INFO_NAMESPACE);

#[derive(Serialize, Deserialize)]
#[deprecated(since = "0.15.0")]
pub struct ContractInfo {
    pub name: String,
    pub definition: String,
    pub version: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub executors: Vec<Addr>,
    pub issuers: Vec<Addr>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
    pub price_precision: Uint128,
    pub size_increment: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[deprecated(since = "0.15.1")]
pub struct ContractInfoV1 {
    pub name: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub approvers: Vec<Addr>,
    pub executors: Vec<Addr>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
    pub price_precision: Uint128,
    pub size_increment: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ContractInfoV2 {
    pub name: String,
    pub bind_name: String,
    pub base_denom: String,
    pub convertible_base_denoms: Vec<String>,
    pub supported_quote_denoms: Vec<String>,
    pub approvers: Vec<Addr>,
    pub executors: Vec<Addr>,
    pub fee_rate: Option<String>,
    pub fee_account: Option<Addr>,
    pub ask_required_attributes: Vec<String>,
    pub bid_required_attributes: Vec<String>,
    pub price_precision: Uint128,
    pub size_increment: Uint128,
}

#[allow(deprecated)]
impl From<ContractInfo> for ContractInfoV2 {
    fn from(contract_info: ContractInfo) -> Self {
        ContractInfoV2 {
            name: contract_info.name,
            bind_name: contract_info.bind_name,
            base_denom: contract_info.base_denom,
            convertible_base_denoms: contract_info.convertible_base_denoms,
            supported_quote_denoms: contract_info.supported_quote_denoms,
            approvers: vec![],
            executors: contract_info.executors,
            fee_rate: None,
            fee_account: None,
            ask_required_attributes: contract_info.ask_required_attributes,
            bid_required_attributes: contract_info.bid_required_attributes,
            price_precision: contract_info.price_precision,
            size_increment: contract_info.size_increment,
        }
    }
}

#[allow(deprecated)]
impl From<ContractInfoV1> for ContractInfoV2 {
    fn from(contract_info: ContractInfoV1) -> Self {
        ContractInfoV2 {
            name: contract_info.name,
            bind_name: contract_info.bind_name,
            base_denom: contract_info.base_denom,
            convertible_base_denoms: contract_info.convertible_base_denoms,
            supported_quote_denoms: contract_info.supported_quote_denoms,
            approvers: contract_info.approvers,
            executors: contract_info.executors,
            fee_rate: None,
            fee_account: None,
            ask_required_attributes: contract_info.ask_required_attributes,
            bid_required_attributes: contract_info.bid_required_attributes,
            price_precision: contract_info.price_precision,
            size_increment: contract_info.size_increment,
        }
    }
}

pub fn set_contract_info(
    store: &mut dyn Storage,
    contract_info: &ContractInfoV2,
) -> Result<(), ContractError> {
    CONTRACT_INFO_V2
        .save(store, &contract_info)
        .map_err(ContractError::Std)
}

pub fn get_contract_info(store: &dyn Storage) -> Result<ContractInfoV2, ContractError> {
    CONTRACT_INFO_V2.load(store).map_err(ContractError::Std)
}

#[allow(deprecated)]
pub fn get_legacy_contract_info(store: &dyn Storage) -> Result<ContractInfo, ContractError> {
    CONTRACT_INFO.load(store).map_err(ContractError::Std)
}

#[cfg(test)]
#[allow(deprecated)]
pub fn set_legacy_contract_info(
    store: &mut dyn Storage,
    contract_info: &ContractInfo,
) -> Result<(), ContractError> {
    CONTRACT_INFO
        .save(store, &contract_info)
        .map_err(ContractError::Std)
}

pub fn migrate_contract_info(
    store: &mut dyn Storage,
    api: &dyn Api,
    msg: &MigrateMsg,
) -> Result<ContractInfoV2, ContractError> {
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // migration from pre 0.15.0
    if VersionReq::parse("<0.15.0")?.matches(&current_version) {
        let mut contract_info_v2: ContractInfoV2 = CONTRACT_INFO.load(store)?.into();

        for approver in &msg.approvers.clone().unwrap_or_default() {
            contract_info_v2
                .approvers
                .push(api.addr_validate(&approver)?)
        }

        contract_info_v2.fee_rate = msg.fee_rate.clone();
        contract_info_v2.fee_account = match &msg.fee_account {
            Some(fee_account) => Some(api.addr_validate(fee_account)?),
            None => None,
        };

        set_contract_info(store, &contract_info_v2)?;
    }

    // migration from 0.15.0 - 0.15.1 => 0.15.2
    if VersionReq::parse(">=0.15.0, <0.15.2")?.matches(&current_version) {
        let mut contract_info_v2: ContractInfoV2 = CONTRACT_INFO_V1.load(store)?.into();

        contract_info_v2.fee_rate = msg.fee_rate.clone();
        contract_info_v2.fee_account = match &msg.fee_account {
            Some(fee_account) => Some(api.addr_validate(fee_account)?),
            None => None,
        };

        set_contract_info(store, &contract_info_v2)?;
    }

    get_contract_info(store)
}

#[cfg(test)]
mod tests {
    use provwasm_mocks::mock_dependencies;

    #[allow(deprecated)]
    use crate::contract_info::{
        get_contract_info, migrate_contract_info, set_contract_info, ContractInfo, ContractInfoV1,
        CONTRACT_INFO,
    };
    use crate::contract_info::{ContractInfoV2, CONTRACT_INFO_V1};
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1};
    use cosmwasm_std::{Addr, Uint128};

    #[test]
    pub fn set_contract_info_with_valid_data() -> Result<(), ContractError> {
        let mut deps = mock_dependencies(&[]);

        set_contract_info(
            &mut deps.storage,
            &ContractInfoV2 {
                name: "contract_name".into(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quo_base_1".into(), "quo_base_2".into()],
                approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                fee_rate: None,
                fee_account: None,
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                price_precision: Uint128(3),
                size_increment: Uint128(1000),
            },
        )?;

        let contract_info = get_contract_info(&deps.storage)?;

        assert_eq!(contract_info.name, "contract_name");
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
            contract_info.approvers,
            vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")]
        );
        assert_eq!(
            contract_info.executors,
            vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")]
        );
        assert_eq!(
            contract_info.ask_required_attributes,
            vec!["ask_tag_1", "ask_tag_2"]
        );
        assert_eq!(
            contract_info.bid_required_attributes,
            vec!["ask_tag_1", "ask_tag_2"]
        );
        assert_eq!(contract_info.price_precision, Uint128(3));
        assert_eq!(contract_info.size_increment, Uint128(1000));

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn pre_0_15_0_migrate_with_existing_issuers() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        CONTRACT_INFO.save(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "contract_def".to_string(),
                version: "0.0.1".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                issuers: vec![Addr::unchecked("issuer_1"), Addr::unchecked("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        )?;

        // migrate without approvers
        migrate_contract_info(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                fee_rate: None,
                fee_account: None,
            },
        )?;

        // verify contract_info updated
        let contract_info = get_contract_info(&deps.storage)?;

        let expected_contract_info = ContractInfoV2 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            fee_rate: None,
            fee_account: None,
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn pre_0_15_0_migrate_with_approvers_and_fees() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        CONTRACT_INFO.save(
            &mut deps.storage,
            &ContractInfo {
                name: "contract_name".into(),
                definition: "contract_definition".to_string(),
                version: "0.14.1".to_string(),
                bind_name: "contract_bind_name".into(),
                base_denom: "base_denom".into(),
                convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
                supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
                executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
                issuers: vec![Addr::unchecked("issuer_1"), Addr::unchecked("issuer_2")],
                ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
                bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
                price_precision: Uint128(2),
                size_increment: Uint128(100),
            },
        )?;

        // migrate with approvers, fee, fee_account
        migrate_contract_info(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: Some(vec!["approver_1".into(), "approver_2".into()]),
                fee_rate: Some("0.01".into()),
                fee_account: Some("fee_account".into()),
            },
        )?;

        // verify contract_info updated
        let contract_info = get_contract_info(&deps.storage)?;

        let expected_contract_info = ContractInfoV2 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            fee_rate: Some("0.01".into()),
            fee_account: Some(Addr::unchecked("fee_account")),
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn migrate_0_15_0_with_fees() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.15.0".to_string(),
            },
        )?;

        CONTRACT_INFO_V1.save(
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
        )?;

        // migrate with fees
        migrate_contract_info(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                fee_rate: Some("0.01".into()),
                fee_account: Some("fee_account".into()),
            },
        )?;

        // verify contract_info updated
        let contract_info = get_contract_info(&deps.storage)?;
        let expected_contract_info = ContractInfoV2 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            fee_rate: Some("0.01".into()),
            fee_account: Some(Addr::unchecked("fee_account")),
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }

    #[test]
    #[allow(deprecated)]
    fn migrate_0_15_0_without_fees() -> Result<(), ContractError> {
        // setup
        let mut deps = mock_dependencies(&[]);

        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.15.0".to_string(),
            },
        )?;

        CONTRACT_INFO_V1.save(
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
        )?;

        // migrate without fees
        migrate_contract_info(
            &mut deps.storage,
            &deps.api,
            &MigrateMsg {
                approvers: None,
                fee_rate: None,
                fee_account: None,
            },
        )?;

        // verify contract_info updated
        let contract_info = get_contract_info(&deps.storage).unwrap();
        let expected_contract_info = ContractInfoV2 {
            name: "contract_name".into(),
            bind_name: "contract_bind_name".into(),
            base_denom: "base_denom".into(),
            convertible_base_denoms: vec!["con_base_1".into(), "con_base_2".into()],
            supported_quote_denoms: vec!["quote_1".into(), "quote_2".into()],
            approvers: vec![Addr::unchecked("approver_1"), Addr::unchecked("approver_2")],
            executors: vec![Addr::unchecked("exec_1"), Addr::unchecked("exec_2")],
            fee_rate: None,
            fee_account: None,
            ask_required_attributes: vec!["ask_tag_1".into(), "ask_tag_2".into()],
            bid_required_attributes: vec!["bid_tag_1".into(), "bid_tag_2".into()],
            price_precision: Uint128(2),
            size_increment: Uint128(100),
        };

        assert_eq!(contract_info, expected_contract_info);

        Ok(())
    }
}
