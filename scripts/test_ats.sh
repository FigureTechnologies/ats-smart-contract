#!/bin/bash -e

# This script stores and instantiates the scope smart contract for the metadata module
export PROV_CMD="./bin/provenanced"

export validator=$("$PROV_CMD" keys show -a validator --keyring-backend test --testnet)

"$PROV_CMD" tx marker new 1000gme.local \
    --type COIN \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker grant "$validator" gme.local mint,burn,admin,withdraw,deposit \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker finalize gme.local \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker activate gme.local \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker new 1000usd.local \
    --type COIN \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker grant "$validator" usd.local mint,burn,admin,withdraw,deposit \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker finalize usd.local \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker activate usd.local \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

## 3. Create Accounts

"$PROV_CMD" keys add buyer \
    --keyring-backend test \
    --testnet

"$PROV_CMD" keys add seller \
    --keyring-backend test \
    --testnet

export buyer=$("$PROV_CMD" keys show -a buyer --keyring-backend test --testnet)
export seller=$("$PROV_CMD" keys show -a seller --keyring-backend test --testnet)

## 4. Fund the accounts
"$PROV_CMD" tx bank send \
    "$validator" \
    "$buyer" \
    100000000000nhash \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker withdraw usd.local 1000usd.local "$buyer" \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx bank send \
    "$validator" \
    "$seller" \
    100000000000nhash \
    --from validator \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

"$PROV_CMD" tx marker withdraw gme.local 500gme.local "$seller" \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

## 5. Store and Instantiate the `ats-smart-contract`
"$PROV_CMD" tx wasm store ./artifacts/ats_smart_contract.wasm \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 1.1 \
    --testnet \
    --yes

"$PROV_CMD" tx wasm instantiate 1 \
'{"name":"ats-ex", "bind_name":"ats-ex.pb", "base_denom":"gme.local", "convertible_base_denoms":[], "supported_quote_denoms":["usd.local"], "approvers":[], "executors":["'$validator'"], "ask_required_attributes":[], "bid_required_attributes":[], "price_precision": "0", "size_increment": "1"}' \
    --admin="$validator" \
    --label ats-ex \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

# Query for the contract address so we can execute it
export contract=$("$PROV_CMD" query wasm list-contract-by-code 1 -t -o json | jq -r ".contracts[0]")

## 6. Create an `ask` order
"$PROV_CMD" tx wasm execute "$contract" \
    '{"create_ask":{"id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "base":"gme.local", "quote":"usd.local", "price": "2", "size":"500"}}' \
    --from="$seller" \
    --amount 500gme.local \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

## 7. Create a `bid` order
"$PROV_CMD" tx wasm execute "$contract" \
    '{"create_bid":{"id":"6a25ffc2-181e-4187-9ac6-572c17038277", "base":"gme.local", "price": "2", "quote":"usd.local", "quote_size":"1000", "size":"500"}}' \
    --amount 1000usd.local \
    --from="$buyer" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

## 8. Match and execute the `ask` and `bid` orders
"$PROV_CMD" tx wasm execute "$contract" \
    '{"execute_match":{"ask_id":"02ee2ed1-939d-40ed-9e1b-bb96f76f0fca", "bid_id":"6a25ffc2-181e-4187-9ac6-572c17038277", "price":"2", "size": "500"}}' \
    --from="$validator" \
    --keyring-backend test \
    --gas auto --gas-prices 1905nhash --gas-adjustment 2 \
    --testnet \
    --yes

## 9. Query account balances to verify trade has executed

# order of the arrays is not guaranteed so we have to check both to verify that we get the correct custom
# denom and not the nhash value.
export buyer_denom=$("$PROV_CMD" q bank balances "$buyer" --testnet | jq -r ".balances[0].denom")
export buyer_denom2=$("$PROV_CMD" q bank balances "$buyer" --testnet | jq -r ".balances[1].denom")
export seller_denom=$("$PROV_CMD" q bank balances "$seller" --testnet | jq -r ".balances[0].denom")
export seller_denom2=$("$PROV_CMD" q bank balances "$seller" --testnet | jq -r ".balances[1].denom")

# verify correct denom
if [ "$buyer_denom" != "gme.local" ] && [ "$buyer_denom2" != "gme.local" ]; then
  echo "The buyer did not get gme.local currency"
  exit 1
fi

if [ "$seller_denom" != "usd.local" ] && [ "$seller_denom2" != "usd.local" ]; then
  echo "The seller did not get usd.local currency"
  exit 1
fi

# verify correct balances
export buyer_balance=$("$PROV_CMD" q bank balances "$buyer" --testnet | jq -r ".balances[0].amount")
export buyer_balance2=$("$PROV_CMD" q bank balances "$buyer" --testnet | jq -r ".balances[1].amount")
export seller_balance=$("$PROV_CMD" q bank balances "$seller" --testnet | jq -r ".balances[0].amount")
export seller_balance2=$("$PROV_CMD" q bank balances "$seller" --testnet | jq -r ".balances[1].amount")

if [ "$buyer_balance" != "500" ] && [ "$buyer_balance2" != "500" ]; then
  echo "The buyer did not the expected amount of 500 but instead got: $buyer_balance and $buyer_balance2"
  exit 1
fi

if [ "$seller_balance" != "1000" ] && [ "$seller_balance2" != "1000" ]; then
  echo "The seller did not get the expected amount of 1000 but instead got: $seller_balance and $seller_balance2"
  exit 1
fi