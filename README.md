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
|------------------|---------|
| 0.17.5           | 241     |
| 0.17.4           | 239     |
| 0.17.3           | 229     |
| 0.17.2           | 152     |
| 0.17.1           | 139     |
| 0.17.0           | 126     |
| 0.16.3           | 110     |
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
### Intel chip (x86)

1. Compile and package to wasm

    ```bash
    make
    ```

### Arm chip (M1 Mac)
1. Compile and package to wasm

   ```bash
   make all-arm
   ```

_NOTE: You must deploy the x86 version because the Arm version produces different artifacts._

- Reference: https://github.com/CosmWasm/rust-optimizer#notice
   - "Arm images are released to ease development and testing on Mac M1 machines. For release / production use, only contracts built with the Intel optimizers must be used."

## Usage

Below is a demonstration on how to:

* [set up Provenance to run locally](#1-blockchain-setup)
* [set up base and quote denominations](#2-create-markers)
* [create buyer and seller accounts](#3-create-accounts)
* [fund said accounts with the created denominations](#4-fund-the-accounts)
* [store and instantiate the smart contract](#5-store-and-instantiate-the-ats-smart-contract)
* [execute an ask order](#6-create-an-ask-order)
* [execute a bid order](#7-create-a-bid-order)
* [execute an order match](#8-match-and-execute-the-ask-and-bid-orders)

_NOTE: Address bech32 values may vary._

### 1. Blockchain setup

1. See the [glossary](#glossary) for an explanation of terms used in this document.

2. Add `GOPATH` to `$HOME/.bashrc`.
   See https://stackoverflow.com/questions/21001387/how-do-i-set-the-gopath-environment-variable-on-ubuntu-what-file-must-i-edit

3. Checkout [provenance v1.14.1](https://github.com/provenance-io/provenance/releases/tag/v1.14.1), clear all existing state, install the `provenanced` command, and start localnet:

    ```bash
    git clone https://github.com/provenance-io/provenance.git
    git checkout v1.14.1
    make clean
    make build
    make install
    # Run the blockchain locally (this is an independent network)
    # `localnet-start` runs 4 nodes, `run` runs 1 node
    make localnet-start OR make run
    ```
#### Please make sure you are under provenance dir before continue
4. Set the directory of the Provenance node you will be communicating with (private keys will be stored here as well):

    ```bash
    export PIO_HOME=<<where your provenance project located, without double quotes>>
    ```
    ```bash
    # If using `make localnet-start`
    export PIO_NODE="$PIO_HOME/build/node0"
   
    # If using `make run`
    export PIO_NODE="$PIO_HOME/build/run/provenanced"
    ```

5. Set an environment variable for the validator node (`node0`) of your local Provenance network:

    ```bash
    # If using `make localnet-start`:
    export NODE0=$(provenanced keys show -a node0 --home $PIO_NODE --keyring-backend test --testnet)

    # If using `make run`:
    export NODE0=$(provenanced keys show -a validator --home $PIO_NODE --keyring-backend test --testnet)
    ```

6. Set an environment variable for the `chain-id` argument:

    ```bash
    # If using `make localnet-start`:
    export CHAIN_ID="chain-local"

    # If using `make run`:
    export CHAIN_ID="testing"
    ```

7. Set node argument:

    ```bash
    export NODE="tcp://localhost:26657"
    ```

_NOTE: You can use `export -p` to check if all the values above set correctly before moving forward._

### 2. Create markers

The example requires two markers: base (gme) and quote (usd).

1. Set up the marker for the base denomination `gme.local`:

    ```bash
    provenanced tx marker new "1000gme.local" \
        --type COIN \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --fees 100381000000nhash \
        --testnet \
        --yes

    provenanced tx marker grant "$NODE0" "gme.local" "mint,burn,admin,withdraw,deposit" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker finalize "gme.local" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker activate "gme.local" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes
    ```

2. Set up marker for the quote denomination `usd.local`:

    ```bash
    provenanced tx marker new "1000usd.local" \
        --type COIN \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --fees 100381000000nhash \
        --testnet \
        --yes

    provenanced tx marker grant "$NODE0" "usd.local" "mint,burn,admin,withdraw,deposit" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker finalize "usd.local" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker activate "usd.local" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes
    ```

### 3. Create accounts

The example requires two trading accounts: `buyer` and `seller`:

1. Create the `buyer` account:

    ```bash
    provenanced keys add buyer \
        --home "$PIO_HOME" \
        --keyring-backend test \
        --testnet
    ```

2. Create the `seller` account:

    ```shell
    provenanced keys add seller \
        --home "$PIO_HOME" \
        --keyring-backend test \
        --testnet
    ```

3. Store the `buyer` and `seller` account addresses:

    ```bash
    export BUYER=$(provenanced keys show -a "buyer" --home "$PIO_HOME" --keyring-backend test --testnet)
    export SELLER=$(provenanced keys show -a "seller" --home "$PIO_HOME" --keyring-backend test --testnet)
    ```

### 4. Fund the accounts

1. Fund the `buyer` account with `nhash` for transaction fees and `usd.local` (quote):

    ```bash
    provenanced tx bank send "$NODE0" "$BUYER" 100000000000nhash \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker withdraw "usd.local" "1000usd.local" "$BUYER" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes
    ```

   Check the `buyer` account balance:
    ```bash
    provenanced q bank balances "$BUYER" \
      --chain-id "$CHAIN_ID" \
      --node "$NODE" \
      --testnet
    ```

2. Fund the `seller` account with nhash for transaction fees and `gme.local` (base) to sell:

    ```bash
    provenanced tx bank send "$NODE0" "$SELLER" 100000000000nhash \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes

    provenanced tx marker withdraw "gme.local" "500gme.local" "$SELLER" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --keyring-backend test \
        --chain-id "$CHAIN_ID" \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes
    ```
   Check the `seller` account balance:

    ```bash
    provenanced q bank balances "$SELLER" \
      --chain-id "$CHAIN_ID" \
      --node "$NODE" \
      --testnet
    ```

### 5. Store and Instantiate the `ats-smart-contract`

1. Copy the previously built `artifacts/ats_smart_contract.wasm` to the root directory of the Provenance git project:

    ```bash
    cp <<Your sc project dir>>/ats-smart-contract/artifacts/ats_smart_contract.wasm "$PIO_HOME"
    cd "$PIO_HOME"
    ```

2. Deploy the `ats-smart-contract` to Provenance and store the resulting code ID:

    ```bash
    # under provenance direct run
    provenanced tx wasm store "ats_smart_contract.wasm" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 1.2 \
        --testnet \
        --yes \
        --output json | jq
    ```
   Find code_id in the result and set
   ```bash
   $ export CODE_ID=<<code_id>>
   ```

   <details>
      <summary>NOTE: advanced</summary>
   
      ```bash
        store_result=$(provenanced tx wasm store "ats_smart_contract.wasm" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 1.2 \
        --testnet \
        --yes \
        --output json | jq)
      ```
      then export code_id

      ```bash
        export CODE_ID=$(jq '.logs[0].events[] | select(.type == "store_code").attributes[] | select(.key == "code_id").value | tonumber' <<< "$store_result")
      ```
   </details>

3. Instantiate the contract

    ```bash
    provenanced tx wasm instantiate "$CODE_ID" \
       '{"name":"ats-ex", "base_denom":"gme.local", "convertible_base_denoms":[], "supported_quote_denoms":["usd.local"], "approvers":[], "executors":["'$NODE0'"], "ask_required_attributes":[], "bid_required_attributes":[], "price_precision": "0", "size_increment": "1"}' \
        --admin "$NODE0" \
        --label "ats-ex" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 1.2 \
        --testnet \
        --yes \
        --output json | jq
    ```

   <details>
      <summary>NOTE: advanced</summary>
   
      ```bash
       instantiate_result=$(provenanced tx wasm instantiate "$CODE_ID" \
       '{"name":"ats-ex", "base_denom":"gme.local", "convertible_base_denoms":[], "supported_quote_denoms":["usd.local"], "approvers":[], "executors":["'$NODE0'"], "ask_required_attributes":[], "bid_required_attributes":[], "price_precision": "0", "size_increment": "1"}' \
        --admin "$NODE0" \
        --label "ats-ex" \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 1.2 \
        --testnet \
        --yes \
        --output json | jq)
      ```
      then export contract_address
      ```bash
        export CONTRACT_ADDRESS=$(jq --raw-output '.logs[0].events[] | select(.type == "instantiate").attributes[] | select(.key == "_contract_address").value' <<< "$instantiate_result")
      ```
   </details>

4. Get the address of the instantiated contract.

   _NOTE: the result when you instantiate sc in step 3 contain the contract_address. OR, `provenanced query wasm list-contracts-by-code $CODE_ID` and get the last one._

   Then do:

   ```bash
   $ export CONTRACT_ADDRESS=<<contract_address>>
   ```

### 6. Create an `ask` order

```bash
provenanced tx wasm execute "$CONTRACT_ADDRESS" \
    '{"create_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "base":"gme.local", "quote":"usd.local", "price": "2", "size":"500"}}' \
    --from seller \
    --amount "500gme.local" \
    --node "$NODE" \
    --home "$PIO_HOME" \
    --chain-id "$CHAIN_ID" \
    --keyring-backend test \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes
```

   Check ask order information

```bash
provenanced query wasm contract-state smart "$CONTRACT_ADDRESS" \
  --node "$NODE" \
  '{"get_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
  --testnet
```
### 7. Create a `bid` order

```bash
provenanced tx wasm execute "$CONTRACT_ADDRESS" \
    '{"create_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277", "base":"gme.local", "price": "2", "quote":"usd.local", "quote_size":"1000", "size":"500"}}' \
    --amount "1000usd.local" \
    --from buyer \
    --node "$NODE" \
    --home "$PIO_HOME" \
    --chain-id "$CHAIN_ID" \
    --keyring-backend test \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes
```

   Check bid order information

```bash
provenanced query wasm contract-state smart "$CONTRACT_ADDRESS" \
  --node "$NODE" \
  '{"get_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
  --testnet
```

### 8. Match and execute the `ask` and `bid` orders

```bash
provenanced tx wasm execute "$CONTRACT_ADDRESS" \
    '{"execute_match":{"ask_id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "bid_id":"6a25ffc2-181e-4187-9ac6-572c17038277", "price":"2", "size": "500"}}' \
    --from "$NODE0" \
    --node "$NODE" \
    --home "$PIO_NODE" \
    --chain-id "$CHAIN_ID" \
    --keyring-backend test \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes
```

### 9. Query account balances to verify trade has executed

1. Check the `buyer` account balance again:
    ```bash
    provenanced q bank balances "$BUYER" \
      --chain-id "$CHAIN_ID" \
      --node "$NODE" \
      --testnet
    ```

2. Check the `seller` account balance again:

    ```bash
    provenanced q bank balances "$SELLER" \
      --chain-id "$CHAIN_ID" \
      --node "$NODE" \
      --testnet
    ```

## Contract Queries
### Contracts list
```bash
$  provenanced query wasm list-contract-by-code $CODE_ID --node "$NODE" -t -o json | jq
```

### Contract general information

```bash
$ provenanced query wasm contract-state smart "$CONTRACT_ADDRESS" \
  --node "$NODE" \
  '{"get_contract_info":{}}' \
  --testnet
```

### Contract version

```bash
$ provenanced query wasm contract-state smart "$CONTRACT_ADDRESS" \
  '{"get_version_info":{}}' \
  --node "$NODE" \
  --testnet
```

## Other actions

### Cancel an ask order

```bash
$ provenanced tx wasm execute "$CONTRACT_ADDRESS" \
    '{"cancel_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca"}}' \
    --from seller \
    --node "$NODE" \
    --home "$PIO_HOME" \
    --chain-id "$CHAIN_ID" \
    --keyring-backend test \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes
```

### Cancel a bid order

```bash
$ provenanced tx wasm execute "$CONTRACT_ADDRESS" \
    '{"cancel_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277"}}' \
    --from buyer \
    --node "$NODE" \
    --home "$PIO_HOME" \
    --chain-id "$CHAIN_ID" \
    --keyring-backend test \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes
```

## Migrate/Upgrade contract

1. Store the new `ats-smart-contract` wasm and extract the resulting code ID:

	```bash
    $ store_result = $(provenanced tx wasm store ats_smart_contract.wasm \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
        --testnet \
        --yes)
    $ export CODE_ID=$(jq '.logs[0].events[] | select(.type == "store_code").attributes[] | select(.key == "code_id").value | tonumber' <<< "$store_result")
    ```

2. Migrate/upgrade to the new code ID:

	```bash
    $ provenanced tx wasm migrate "$CONTRACT_ADDRESS" "$CODE_ID" \
    '{"migrate":{}}' \
        --from "$NODE0" \
        --node "$NODE" \
        --home "$PIO_NODE" \
        --chain-id "$CHAIN_ID" \
        --keyring-backend test \
        --gas auto \
        --gas-prices 1905nhash \
        --gas-adjustment 2 \
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
