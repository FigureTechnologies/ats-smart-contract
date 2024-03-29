use crate::contract_info::require_version;
use crate::error::ContractError;
use crate::msg::MigrateMsg;
use crate::version_info::get_version_info;
use cosmwasm_std::{Addr, Coin, DepsMut, Uint128};
use cw_storage_plus::Map;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub const NAMESPACE_ORDER_ASK: &str = "ask";

pub const ASKS_V1: Map<&[u8], AskOrderV1> = Map::new(NAMESPACE_ORDER_ASK);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderStatus {
    PendingIssuerApproval,
    Ready {
        approver: Addr,
        converted_base: Coin,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AskOrderClass {
    Basic,
    Convertible { status: AskOrderStatus },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AskOrderV1 {
    pub id: String,
    pub owner: Addr,
    pub class: AskOrderClass,
    pub base: String,
    pub quote: String,
    pub price: String,
    pub size: Uint128,
}

pub fn migrate_ask_orders(deps: DepsMut, _msg: &MigrateMsg) -> Result<(), ContractError> {
    let store = deps.storage;
    let version_info = get_version_info(store)?;
    let current_version = Version::parse(&version_info.version)?;

    // The last version of ask order (`AskOrderV1`) was introduced in 0.15.0:
    require_version(">=0.15.0", &current_version)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::migrate_ask_orders;
    use crate::error::ContractError;
    use crate::msg::MigrateMsg;
    use crate::version_info::{set_version_info, VersionInfoV1, CRATE_NAME};
    use provwasm_mocks::mock_provenance_dependencies;

    #[test]
    pub fn ask_migration_fails_if_contract_is_too_old() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        // Contract too old:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.14.9".to_string(),
            },
        )?;

        let result = migrate_ask_orders(
            deps.as_mut(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
        );

        match result {
            Ok(_) => panic!("expected error, but ok"),
            Err(error) => match error {
                ContractError::UnsupportedUpgrade {
                    source_version,
                    target_version,
                } => {
                    assert_eq!(source_version, "0.14.9");
                    assert_eq!(target_version, ">=0.15.0");
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        Ok(())
    }

    #[test]
    pub fn ask_migration_minimum_version_check() -> Result<(), ContractError> {
        // Setup
        let mut deps = mock_provenance_dependencies();

        // Contract minimum version:
        set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: CRATE_NAME.to_string(),
                version: "0.15.0".to_string(),
            },
        )?;

        let result = migrate_ask_orders(
            deps.as_mut(),
            &MigrateMsg {
                approvers: None,
                ask_fee_rate: None,
                ask_fee_account: None,
                bid_fee_rate: None,
                bid_fee_account: None,
                ask_required_attributes: None,
                bid_required_attributes: None,
            },
        );

        assert!(result.is_ok());

        Ok(())
    }
}
