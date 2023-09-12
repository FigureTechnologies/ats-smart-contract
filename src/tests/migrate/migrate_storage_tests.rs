#[cfg(test)]
mod migrate_tests {
    use crate::ask_order::{AskOrderClass, AskOrderStatus, AskOrderV1, ASKS_V1};
    use crate::bid_order::{BidOrderV3, BIDS_V3};
    use crate::version_info::{set_version_info, VersionInfoV1};
    use cosmwasm_std::{Addr, Coin, Storage, Uint128};
    use cosmwasm_storage::{bucket, bucket_read, Bucket};
    use provwasm_mocks::mock_dependencies;
    use serde::de::DeserializeOwned;
    use serde::Serialize;

    static CONTRACT_DEFINITION: &str = "ats-smart-contract";
    static CONTRACT_VERSION: &str = "0.19.2";
    static LEGACY_NAMESPACE_ORDER_ASK: &[u8] = b"ask";
    static LEGACY_NAMESPACE_ORDER_BID: &[u8] = b"bid";

    fn get_ask_storage<T>(storage: &mut dyn Storage) -> Bucket<T>
    where
        T: Serialize + DeserializeOwned,
    {
        bucket(storage, LEGACY_NAMESPACE_ORDER_ASK)
    }

    fn get_bid_storage<T>(storage: &mut dyn Storage) -> Bucket<T>
    where
        T: Serialize + DeserializeOwned,
    {
        bucket(storage, LEGACY_NAMESPACE_ORDER_BID)
    }

    #[test]
    fn store_bid_with_bucket_then_read_with_map() {
        let mut deps = mock_dependencies(&[]);

        let test_bid: BidOrderV3 = BidOrderV3 {
            id: "c13f8888-ca43-4a64-ab1b-1ca8d60aa49b".into(),
            owner: Addr::unchecked("bidder"),
            base: Coin {
                amount: Uint128::new(200),
                denom: "base_1".into(),
            },
            accumulated_base: Uint128::new(100),
            accumulated_quote: Uint128::new(200),
            accumulated_fee: Uint128::zero(),
            fee: None,
            quote: Coin {
                amount: Uint128::new(400),
                denom: "quote_1".into(),
            },
            price: "2".into(),
        };
        // Store ask using Bucket
        let mut bid_storage_rw: Bucket<BidOrderV3> = get_bid_storage(&mut deps.storage);
        bid_storage_rw
            .save(test_bid.id.as_bytes(), &test_bid)
            .unwrap();

        // Load bid using Map
        let bid_from_storage_result = BIDS_V3.load(&mut deps.storage, test_bid.id.as_bytes());
        match bid_from_storage_result {
            Ok(bid_from_storage) => {
                assert_eq!(bid_from_storage, test_bid);
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }

    #[test]
    fn store_ask_with_bucket_then_read_with_map() {
        let mut deps = mock_dependencies(&[]);

        let test_ask: AskOrderV1 = AskOrderV1 {
            base: "con_base_1".into(),
            class: AskOrderClass::Convertible {
                status: AskOrderStatus::PendingIssuerApproval,
            },
            id: "ab5f5a62-f6fc-46d1-aa84-51ccc51ec367".into(),
            owner: Addr::unchecked("asker"),
            price: "2".into(),
            quote: "quote_1".into(),
            size: Uint128::new(100),
        };
        // Store ask using Bucket
        let mut ask_storage_rw: Bucket<AskOrderV1> = get_ask_storage(&mut deps.storage);
        ask_storage_rw
            .save(test_ask.id.as_bytes(), &test_ask)
            .unwrap();

        // Load ask using Map
        let ask_from_storage_result = ASKS_V1.load(&mut deps.storage, test_ask.id.as_bytes());
        match ask_from_storage_result {
            Ok(ask_from_storage) => {
                assert_eq!(ask_from_storage, test_ask);
            }
            Err(error) => panic!("unexpected error: {:?}", error),
        }
    }
}
