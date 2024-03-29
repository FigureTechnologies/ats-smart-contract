use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use ats_smart_contract::ask_order::{AskOrderClass, AskOrderStatus, AskOrderV1};
#[allow(deprecated)]
use ats_smart_contract::bid_order::{BidOrderV2, BidOrderV3};
use ats_smart_contract::contract_info::ContractInfoV3;
use ats_smart_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use ats_smart_contract::version_info::VersionInfoV1;

#[allow(deprecated)]
fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(AskOrderV1), &out_dir);
    export_schema(&schema_for!(AskOrderClass), &out_dir);
    export_schema(&schema_for!(AskOrderStatus), &out_dir);
    export_schema(&schema_for!(BidOrderV2), &out_dir);
    export_schema(&schema_for!(BidOrderV3), &out_dir);
    export_schema(&schema_for!(ContractInfoV3), &out_dir);
    export_schema(&schema_for!(VersionInfoV1), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
}
