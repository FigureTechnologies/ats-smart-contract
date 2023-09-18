use crate::error::ContractError;
use cosmwasm_std::Response;
use prost::Message;
use provwasm_std::shim::Any;
use provwasm_std::types::cosmos::auth::v1beta1::BaseAccount;
use provwasm_std::types::provenance::marker::v1::{
    AccessGrant, MarkerAccount, MarkerStatus, MarkerType, QueryMarkerResponse,
};

pub fn validate_execute_invalid_id_field(execute_response: Result<Response, ContractError>) {
    match execute_response {
        Ok(_) => panic!("expected error, but ok"),
        Err(error) => match error {
            ContractError::InvalidFields { fields } => {
                assert!(fields.contains(&"id".into()));
            }
            error => panic!("unexpected error: {:?}", error),
        },
    }
}

pub fn setup_restricted_asset_marker(
    base_address: String,
    access_address: String,
    marker_denom: String,
) -> QueryMarkerResponse {
    setup_asset_marker(
        base_address,
        access_address,
        marker_denom,
        MarkerType::Restricted,
    )
}

pub fn setup_asset_marker(
    base_address: String,
    access_address: String,
    marker_denom: String,
    marker_type: MarkerType,
) -> QueryMarkerResponse {
    let expected_marker: MarkerAccount = MarkerAccount {
        base_account: Some(BaseAccount {
            address: base_address,
            pub_key: None,
            account_number: 10,
            sequence: 0,
        }),
        manager: "".to_string(),
        access_control: vec![AccessGrant {
            address: access_address,
            permissions: vec![1, 2, 3, 4, 5, 6, 7],
        }],
        status: MarkerStatus::Active.into(),
        denom: marker_denom,
        supply: "1000".to_string(),
        marker_type: marker_type.into(),
        supply_fixed: false,
        allow_governance_control: true,
        allow_forced_transfer: false,
        required_attributes: vec![],
    };

    QueryMarkerResponse {
        marker: Some(Any {
            type_url: "/provenance.marker.v1.MarkerAccount".to_string(),
            value: expected_marker.encode_to_vec(),
        }),
    }
}
