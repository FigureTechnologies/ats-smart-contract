# ATS Smart Contract
A ProvWasm smart contract that provides on-chain services for the Provenance ATS.

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

### [Provenance Testnet](https://github.com/provenance-io/testnet) Deployments
#### [pio-testnet-1](https://github.com/provenance-io/testnet/tree/main/pio-testnet-1)

| Contract Version | Code ID |
| ---------------- | ------- |
| 0.16.0           | 45      |
| 0.15.7           | 40      |
| 0.15.6           | 38      |
| 0.15.5           | 35      |
| 0.15.4           | 34      |
| 0.15.3           | 33      |
| 0.15.2           | 32      |
| 0.14.5           | 23      |
| 0.14.3           | 20      |
| 0.14.2           | 16      |
| 0.14.1           | 14      |

## Build

1. Compile and package to wasm

    ```bash
    make
    ```

## Example Usage

_note: Address bech32 values may vary._

### 1. Blockchain setup

1. Checkout [provenance](https://github.com/provenance-io/provenance) v1.7.2, clear all existing state, install the `provenanced` command, and start.
    
    ```bash
    git clone https://github.com/provenance-io/provenance.git
    git checkout v1.7.2
    make clean
    make build
    make install
    make run
    ```

### 2. Create markers

The example requires two markers: base (gme) and quote (usd)

1. Base

    ```shell
    provenanced tx marker new 1000gme.local \
        --type COIN \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker grant (build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) gme.local mint,burn,admin,withdraw,deposit \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker finalize gme.local \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker activate gme.local \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

2. Quote
   
    ```shell
    provenanced tx marker new 1000usd.local \
        --type COIN \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker grant (build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) usd.local mint,burn,admin,withdraw,deposit \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker finalize usd.local \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    
    provenanced tx marker activate usd.local \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

### 3. Create accounts

The example requires two trading accounts: buyer and seller

1. Buyer

    ```shell
    provenanced keys add buyer \
        --home build/run/provenanced \
        --keyring-backend test \
        --testnet
    ```

2. Seller

    ```shell
    provenanced keys add seller \
        --home build/run/provenanced \
        --keyring-backend test \
        --testnet
    ```

### 4. Fund the accounts

1. Fund buyer's account with nhash for transaction fees and usd (quote)

    ```shell
    provenanced tx bank send \
        (provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) \
        (provenanced keys show -ta buyer --home build/run/provenanced --keyring-backend test) \
        100000000000nhash \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
        
    provenanced tx marker withdraw usd.local 1000usd.local (build/provenanced keys show -ta buyer --home build/run/provenanced --keyring-backend test) \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

2. Fund seller's account with nhash for transaction fees and gme (base) to sell

    ```shell
    provenanced tx bank send \
        (provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) \
        (provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
        100000000000nhash \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
        
    provenanced tx marker withdraw gme.local 500gme.local (build/provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

### 5. Store and Instantiate the `ats-smart-contract`

1. Deploy the `ats-smart-contract` to provenance. Copy the previously built `artifacts/ats-smart-contract.wasm` to the root directory of the Provenance git project
    ```shell
    provenanced tx wasm store ats_smart_contract.wasm \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

2. Instantiate the contract, binding the name `ats-ex.pb` to the contract address:
    ```shell
    provenanced tx wasm instantiate 1 \
   '{"name":"ats-ex", "bind_name":"ats-ex.pb", "base_denom":"gme.local", "convertible_base_denoms":[], "supported_quote_denoms":["usd.local"], "approvers":[], "executors":["'(build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test)'"], "ask_required_attributes":[], "bid_required_attributes":[], "price_precision": "0", "size_increment": "1"}' \
        --admin (build/provenanced keys show -ta validator --home build/run/provenanced --keyring-backend test) \
        --label ats-ex \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

### 6. Create an `ask` order

```shell
provenanced tx wasm execute (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"create_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "base":"gme.local", "quote":"usd.local", "price": "2", "size":"500"}}' \
    --from seller \
    --amount 500gme.local \
    --home build/run/provenanced \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes
```

### 7. Create a `bid` order

```shell
provenanced tx wasm execute (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"create_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277", "base":"gme.local", "price": "2", "quote":"usd.local", "quote_size":"1000", "size":"500"}}' \
    --amount 1000usd.local \
    --from buyer \
    --home build/run/provenanced \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes
```

### 8. Match and execute the `ask` and `bid` orders

```shell
provenanced tx wasm execute (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"execute_match":{"ask_id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "bid_id":"6a25ffc2-181e-4187-9ac6-572c17038277", "price":"2", "size": "500"}}' \
    --from validator \
    --home build/run/provenanced \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes
```

### 9. Query account balances to verify trade has executed

1. Buyer account balance
    ```shell
    provenanced q bank balances \
      (provenanced keys show -ta buyer --home build/run/provenanced --keyring-backend test) \
      --testnet
    ```
   
1. Seller account balance
    ```shell
    provenanced q bank balances \
      (provenanced keys show -ta seller --home build/run/provenanced --keyring-backend test) \
      --testnet
    ```

## Contract Queries

### contract general information

```shell
provenanced query wasm contract-state smart \
    (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"get_contract_info":{}}' --testnet
```

### contract version

```shell
provenanced query wasm contract-state smart \
    (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"get_version_info":{}}' --testnet
```

### ask order information

```shell
provenanced query wasm contract-state smart (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
  '{"get_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
  --testnet
```

### bid order information

```shell
provenanced query wasm contract-state smart (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
  '{"get_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
  --testnet
```

## Other actions

### Cancel an ask order

```shell
provenanced tx wasm execute (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"cancel_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
    --from seller \
    --home build/run/provenanced \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes
```

### Cancel a bid order

```shell
provenanced tx wasm execute (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') \
    '{"cancel_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
    -t \
    --from buyer \
    --home build/run/provenanced \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes
```

## Migrate/Upgrade contract

1. Store the new `ats-smart-contract` wasm

	```shell
    provenanced tx wasm store ats_smart_contract.wasm \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

1. Migrate/Upgrade to the new code id
   
   _note: The `CODE_ID` is the `code_id` returned when storing the new wasm in the previous step._

	```shell
    provenanced tx wasm migrate (provenanced q name resolve ats-ex.pb --testnet | awk '{print $2}') CODE_ID \
    '{"migrate":{}}' \
        --from validator \
        --home build/run/provenanced \
        --keyring-backend test \
        --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
        --testnet \
        --yes
    ```

## Glossary

The following terms are used to identify parts of a token swap. It is strongly influenced by Forex:
- token pair - two different currencies that are traded for each other (ex: BTC/HASH)
- base (token) - the token being sold, indicated by the first token in the pair representation BASE/QUOTE
- quote (token) - the token used to buy the base token, identified by the second token in the pair representation BASE/QUOTE
- ask - the sell order
- bid - the buy order
- match - an ask and bid order pairing
