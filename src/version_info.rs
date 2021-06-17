use crate::error::ContractError;
use cosmwasm_std::Storage;
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
        .save(store, &version_info)
        .map_err(ContractError::Std)
}

pub fn get_version_info(store: &dyn Storage) -> Result<VersionInfoV1, ContractError> {
    VERSION_INFO.load(store).map_err(ContractError::Std)
}

pub fn migrate_version_info(store: &mut dyn Storage) -> Result<VersionInfoV1, ContractError> {
    let version_info = VersionInfoV1 {
        definition: CRATE_NAME.to_string(),
        version: PACKAGE_VERSION.to_string(),
    };

    set_version_info(store, &version_info)?;

    Ok(version_info)
}

#[cfg(test)]
mod tests {
    use crate::version_info::{get_version_info, set_version_info, VersionInfoV1};
    use provwasm_mocks::mock_dependencies;
    use std::cmp::Ordering;

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
                assert_eq!(version_info.version.cmp(&"0.14.10".into()), Ordering::Less);
            }
            result => panic!("unexpected error: {:?}", result),
        }
    }
}
