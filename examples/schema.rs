use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use ats_smart_contract::contract_info::ContractInfo;
use ats_smart_contract::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use ats_smart_contract::state::{AskOrder, AskOrderClass, AskOrderStatus, BidOrder};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(AskOrder), &out_dir);
    export_schema(&schema_for!(AskOrderClass), &out_dir);
    export_schema(&schema_for!(AskOrderStatus), &out_dir);
    export_schema(&schema_for!(BidOrder), &out_dir);
    export_schema(&schema_for!(ContractInfo), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
}
