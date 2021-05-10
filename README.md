# ATS Exchange Smart Contract
A ProvWasm smart contract that provides on-chain services for the ATS Exchange.

## Status
[![Latest Release][release-badge]][release-latest]
[![Build Status][build-badge]][build-status]
[![Code Coverage][codecov-badge]][codecov-report]

[release-badge]: https://img.shields.io/github/v/tag/provenance-io/ats-smart-contract.svg?sort=semver
[release-latest]: https://github.com/provenance-io/ats-smart-contract/releases/latest
[build-badge]: https://github.com/provenance-io/ats-smart-contract/actions/workflows/rust.yaml/badge.svg?branch=main
[build-status]: https://github.com/provenance-io/ats-smart-contract/actions/workflows/rust.yaml
[codecov-badge]: https://codecov.io/gh/provenance-io/ats-smart-contract/branch/main/graph/badge.svg
[codecov-report]: https://codecov.io/gh/provenance-io/ats-smart-contract

### Glossary

The following terms are used to identify parts of a token exchange. It is strongly influenced by Forex:
- token pair - two different currencies that are traded in exchange for each other (ex: BTC/HASH)
- base (token) - the token being sold, indicated by the first token in the pair representation BASE/QUOTE
- quote (token) - the token used to buy the base token, identified by the second token in the pair representation BASE/QUOTE
- ask - the sell order
- bid - the buy order
- match - an ask and bid order pairing

## Build

_Make sure $PIO_HOME is set_

Compile and install

```bash
make
```

## Example Usage
_note: Address bech32 values and other params may vary._

0. Pre-configure the following:
    1. Accounts:
        - asker
        - buyer
    1. Markers:
        - hash
        - gme (base)
        - usd (quote)


0. Store the `ats-smart-contract` WASM:
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
        --fees 40000nhash \
        --broadcast-mode block \
        --yes | jq;
    ```

0. Instantiate the contract, binding the name `ats-exchange.sc.pb` to the contract address:
    ```shell
    build/provenanced tx wasm instantiate 1 \
        '{"name":"ats-gme-usd", "bind_name":"atsgmeusd.sc", "base_denom":"gme","convertible_base_denoms":[],"supported_quote_denoms":["usd"],
        "executors":["'(build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test)'"],"issuers":[],
        "ask_required_attributes":[],"bid_required_attributes":[]}' \
        -t \
        --admin (build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --label ats-gme-usd \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 6000nhash \
        --broadcast-mode block \
        --yes | jq
    ```

0. Create an `ask` order:

    _note: The json data '{"create_ask":{}}' is the action and order data to pass into the smart contract. The actual
   marker token sent is the order base, identified by `--amount` below._

    ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"create_ask":{"id":"ask_id", "price":"2", "quote":"usd"}}' \
        -t \
        --amount 10gme \
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

0. Create a `bid` order:

    _note: The json data '{"create_bid":{}}' is the action and order data to pass into the smart contract, he actual
   marker token sent is the order quote, identified by `--amount` below._

   ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"create_bid":{"id":"bid_id", "base":"gme", "price":"2", "size":"5"}}' \
        -t \
        --amount 10usd \
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

0. Match and execute the ask and bid orders.

   ```shell
    build/provenanced tx wasm execute tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
        '{"execute_match":{"ask_id":"ask_id", "bid_id":"bid_id"}}' \
        -t \
        --from validator \
        --keyring-backend test \
        --home build/run/provenanced \
        --chain-id testing \
        --gas auto \
        --gas-adjustment 1.4 \
        --fees 5000nhash \
        --broadcast-mode block \
        --yes | jq
    ```

## Other actions

Cancel the contract.

```shell
build/provenanced tx wasm execute \
    tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"cancel_ask":{"id":"ask_id"}}' \
    --from (build/provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
    --keyring-backend test \
    --home build/node0 \
    --chain-id chain-local \
    --gas auto \
    --gas-adjustment 1.4 \
    --fees 5000nhash \
    --broadcast-mode block \
    --yes \
    --testnet | jq
```

Query for ask order information:

```shell
build/provenanced query wasm contract-state smart tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"get_ask":{"id":"ask_id"}}' \
    --ascii \
    --testnet
```

Query for bid order information:

```shell
build/provenanced query wasm contract-state smart tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"get_bid":{"id":"bid_id"}}' \
    --ascii \
    --testnet
```

Query for general contract information

```shell
build/provenanced query wasm contract-state smart \
    tp18vd8fpwxzck93qlwghaj6arh4p7c5n89x8kskz \
    '{"get_contract_info":{}}' --testnet
```
