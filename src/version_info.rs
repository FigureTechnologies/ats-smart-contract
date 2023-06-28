use crate::error::ContractError;
use cosmwasm_std::{DepsMut, Storage};
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
const VERSION_INFO_NAMESPACE: &str = "version_info";
const VERSION_INFO: Item<VersionInfoV1> = Item::new(VERSION_INFO_NAMESPACE);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VersionInfoV1 {
    pub definition: String,
    pub version: String,
}

pub fn set_version_info(
    store: &mut dyn Storage,
    version_info: &VersionInfoV1,
) -> Result<(), ContractError> {
    VERSION_INFO
        .save(store, version_info)
        .map_err(ContractError::Std)
}

pub fn get_version_info(store: &dyn Storage) -> Result<VersionInfoV1, ContractError> {
    VERSION_INFO.load(store).map_err(ContractError::Std)
}

pub fn migrate_version_info(
    deps: DepsMut,
) -> Result<VersionInfoV1, ContractError> {
    let version_info = VersionInfoV1 {
        definition: CRATE_NAME.to_string(),
        version: PACKAGE_VERSION.to_string(),
    };

    set_version_info(deps.storage, &version_info)?;

    Ok(version_info)
}

#[cfg(test)]
mod tests {
    use crate::error::ContractError;
    use crate::version_info::{get_version_info, set_version_info, VersionInfoV1};
    use cosmwasm_std::StdError;
    use provwasm_mocks::mock_provenance_dependencies;

    #[test]
    pub fn set_version_info_with_valid_data() {
        let mut deps = mock_provenance_dependencies();
        let result = set_version_info(
            &mut deps.storage,
            &VersionInfoV1 {
                definition: "def".to_string(),
                version: "0.14".to_string(),
            },
        );
        match result {
            Ok(()) => {}
            result => panic!("unexpected error: {:?}", result),
        }

        let version_info = get_version_info(&deps.storage);
        match version_info {
            Ok(version_info) => {
                assert_eq!(version_info.definition, "def");
                assert_eq!(version_info.version, "0.14");
            }
            result => panic!("unexpected error: {:?}", result),
        }
    }

    #[test]
    pub fn version_info_not_found() -> Result<(), ContractError> {
        let deps = mock_provenance_dependencies();

        let version_info = get_version_info(&deps.storage);
        match version_info {
            Ok(_) => {
                panic!("expected error, but ok")
            }
            Err(error) => match error {
                ContractError::Std(StdError::NotFound { kind }) => {
                    assert_eq!(kind, "ats_smart_contract::version_info::VersionInfoV1");
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        Ok(())
    }
}
