# ATS Smart Contract
A ProvWasm smart contract that provides on-chain services for the Provenance ATS.

## Status

---

[![Latest Release][release-badge]][release-latest]
[![Build Status][build-badge]][build-status]
[![Code Coverage][codecov-badge]][codecov-report]

[release-badge]: https://img.shields.io/github/v/tag/provenance-io/ats-smart-contract.svg?sort=semver
[release-latest]: https://github.com/provenance-io/ats-smart-contract/releases/latest
[build-badge]: https://github.com/provenance-io/ats-smart-contract/actions/workflows/rust.yaml/badge.svg?branch=main
[build-status]: https://github.com/provenance-io/ats-smart-contract/actions/workflows/rust.yaml
[codecov-badge]: https://codecov.io/gh/provenance-io/ats-smart-contract/branch/main/graph/badge.svg
[codecov-report]: https://codecov.io/gh/provenance-io/ats-smart-contract

### [Provenance Testnet](https://github.com/provenance-io/testnet) Deployments
####pio-testnet-1

| Contract Version | Code ID |
| ---------------- | ------- |
| 0.14.1           | 14      |
| 0.14.2           | 16      |
| 0.14.3           | 20      |
| 0.14.5           | 23      |

## Build

---

_Make sure $PIO_HOME is set_

1. Compile and install

    ```bash
    make
    ```

## Example Usage

---
_note: Address bech32 values and other params may vary._

0. Pre-configure the following:
    1. Accounts:
        - asker
        - buyer
    1. Markers:
        - hash
        - gme (base)
        - usd (quote)


1. Store the `ats-smart-contract` WASM:
    ```shell
    build/provenanced tx wasm store ats_smart_contract.wasm \
        -t \
        --source "https://github.com/provenance-io/ats-smart-contract" \
        --builder "cosmwasm/rust-optimizer:0.11.3" \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --fees 500000nhash \
        --broadcast-mode block \
        --yes | jq;
    ```

1. Instantiate the contract, binding the name `atsgmeusd.sc` to the contract address:
    ```shell
    build/provenanced tx wasm instantiate 1 '{"name":"ats-ex", "bind_name":"ats-ex.sc", "base_denom":"gme", "convertible_base_denoms":[], "supported_quote_denoms":["usd"], "executors":["'(build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test)'"], "issuers":[], "ask_required_attributes":[], "bid_required_attributes":[], "price_precision": "0", "size_increment": "1"}' \
        -t \
        --admin (build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --label ats-gme-usd \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 7000nhash \
        --broadcast-mode block \
        --yes | jq
    ```

1. Create an `ask` order:

    _note: The json data '{"create_ask":{}}' is the action and order data to pass into the smart contract. The actual
   marker token sent is the order base, identified by `--amount` below._

    ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"create_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "quote":"usd", "price": "1"}}' \
        -t \
        --amount 700gme \
        --from (build/provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 5000nhash \
        --broadcast-mode block \
        --yes | jq
    ```

1. Create a `bid` order:

    _note: The json data '{"create_bid":{}}' is the action and order data to pass into the smart contract, he actual
   marker token sent is the order quote, identified by `--amount` below._

    ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"create_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277", "base":"gme", "size":"500", "price": "2"}}' \
        --amount 1000usd \
        --from (build/provenanced keys show -ta buyer --home build/run/provenanced --keyring-backend test) \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 5000nhash \
        --broadcast-mode block \
        --yes \
        --testnet | jq
    ```

1. Match and execute the ask and bid orders.

    ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"execute_match":{"ask_id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "bid_id":"6a25ffc2-181e-4187-9ac6-572c17038277", "price":"2", "size": "100"}}' \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 6000nhash \
        --broadcast-mode block \
        --yes \
        --testnet | jq
    ```

## Other actions

---
Cancel an ask order:

```shell
build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"cancel_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
    -t \
    --from (build/provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
    --keyring-backend test \
    --home build/run/provenanced \
    --chain-id testing \
    --gas auto \
    --gas-adjustment 1.4 \
    --fees 5000nhash \
    --broadcast-mode block \
    --yes | jq
```

Cancel a bid order:

```shell
build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"cancel_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
    -t \
    --from (build/provenanced keys show -ta buyer --home build/run/provenanced --keyring-backend test) \
    --keyring-backend test \
    --home build/run/provenanced \
    --chain-id testing \
    --gas auto \
    --gas-adjustment 1.4 \
    --fees 5000nhash \
    --broadcast-mode block \
    --yes | jq
```

Query for ask order information:

```shell
build/provenanced query wasm contract-state smart tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
  '{"get_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
  --ascii \
  --testnet
```

Query for bid order information:

```shell
build/provenanced query wasm contract-state smart tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
  '{"get_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
  --ascii \
  --testnet
```

Query for general contract information

```shell
build/provenanced query wasm contract-state smart \
    tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"get_contract_info":{}}' --testnet
```

## Migrate/Upgrade contract

---
1. Store the new `ats-smart-contract` WASM:
    ```shell
    build/provenanced tx wasm store ats_smart_contract.wasm \
        -t \
        --source "https://github.com/provenance-io/ats-smart-contract" \
        --builder "cosmwasm/rust-optimizer:0.11.3" \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --fees 500000nhash \
        --broadcast-mode block \
        --yes | jq;
    ```

1. Migrate/Upgrade to the new code id:
   
   _note: The `CODE_ID` is the `code_id` returned when storing the new wasm in the previous step._

    ```shell
    build/provenanced tx wasm migrate tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz CODE_ID \
    '{"migrate":{}}' \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 6000nhash \
        --broadcast-mode block \
        --yes \
        --testnet | jq
    ```

## Glossary

---
The following terms are used to identify parts of a token swap. It is strongly influenced by Forex:
- token pair - two different currencies that are traded for each other (ex: BTC/HASH)
- base (token) - the token being sold, indicated by the first token in the pair representation BASE/QUOTE
- quote (token) - the token used to buy the base token, identified by the second token in the pair representation BASE/QUOTE
- ask - the sell order
- bid - the buy order
- match - an ask and bid order pairing