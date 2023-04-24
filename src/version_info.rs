use crate::error::ContractError;
use cosmwasm_std::{DepsMut, Storage};
use cw_storage_plus::Item;
use provwasm_std::ProvenanceQuery;
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
    let version_info_result = VERSION_INFO.load(store).map_err(ContractError::Std);
    match &version_info_result {
        Ok(_) => version_info_result,
        // version support added in 0.15.0, all previous versions used ContractInfo for version tracking
        // if VersionInfo doesn't exist, try ContractInfo
        Err(_) => Err(ContractError::UnsupportedUpgrade {
            source_version: "UNKNOWN".to_string(),
            target_version: PACKAGE_VERSION.into(),
        }),
    }
}

pub fn migrate_version_info(
    deps: DepsMut<ProvenanceQuery>,
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
    use crate::version_info::{get_version_info, set_version_info, VersionInfoV1, PACKAGE_VERSION};
    use provwasm_mocks::mock_dependencies;

    #[test]
    pub fn set_version_info_with_valid_data() {
        let mut deps = mock_dependencies(&[]);
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
    pub fn get_version_info_unknown() -> Result<(), ContractError> {
        let deps = mock_dependencies(&[]);

        let version_info = get_version_info(&deps.storage);
        match version_info {
            Ok(_) => {
                panic!("expected error, but ok")
            }
            Err(error) => match error {
                ContractError::UnsupportedUpgrade {
                    source_version,
                    target_version,
                } => {
                    assert_eq!(source_version, "UNKNOWN");
                    assert_eq!(target_version, PACKAGE_VERSION)
                }
                _ => panic!("unexpected error: {:?}", error),
            },
        }

        Ok(())
    }
}
