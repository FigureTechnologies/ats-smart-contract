#!/usr/bin/env bash

# Adapted from https://hackmd.io/TLVyhpL3SFSyHQKdxggduw

###############################################################################
# Expected inputs:
#
# - DERIVATION_PATH - default: "m/44'/1'/0'/0/0'""
# - BUYER_MNEMONIC - default: <generated>
# - SELLER_MNEMONIC - default: <generated>
# - BASE_DENOM - default: "gme.local"
# - BASE_AMOUNT - default: "1000"
# - QUOTE_DENOM - default: "usd.local"
# - QUOTE_AMOUNT - default: "1000"
###############################################################################

set -e

buyer_key_name="buyer.ats-profiler.local"
seller_key_name="seller.ats-profiler.local"

if [ -z "${GOPATH+set}" ] || [ -z "$GOPATH" ]; then
  echo "Set GOPATH before proceeding in $HOME/.profile"
  exit 1
else
  echo "- Using GOPATH=$GOPATH"
fi

if [ -z "${PIO_HOME+set}" ] || [ -z "$PIO_HOME" ]; then
  echo "Missing PIO_HOME"
  exit 1
else
  echo "- Using PIO_HOME=$PIO_HOME"
fi

# Set the dir of the provenance node you will be communicating with.
#   Private keys will be stored here as well.
# If using `make localnet-start`
#   PIO_NODE="$PIO_HOME/build/node0"
# If using `make run`
#   PIO_NODE="$PIO_HOME/build/run/provenanced"
PIO_NODE="$PIO_HOME/build/node0"
echo "- Using PIO_NODE = $PIO_NODE"

if [ -z "${CHAIN_ID+set}" ] || [ -z "$CHAIN_ID" ]; then
  CHAIN_ID="chain-local"
  echo "- Missing CHAIN_ID; using CHAIN_ID=$CHAIN_ID"
fi

node0=$(provenanced keys show -a node0 --home "$PIO_NODE" --keyring-backend test --testnet)
echo "- Using node0=$node0"

if [ -z "${DERIVATION_PATH+set}" ] || [ -z "$DERIVATION_PATH" ]; then
  DERIVATION_PATH="m/44'/1'/0'/0/0'"
  echo "- Missing DERIVATION_PATH; using: $DERIVATION_PATH"
fi

if [ -z "${BUYER_MNEMONIC+set}" ] || [ -z "$BUYER_MNEMONIC" ]; then
  BUYER_MNEMONIC=$(provenanced keys mnemonic)
  echo "- Missing BUYER_MNEMONIC; using:"
  echo
  echo "    $BUYER_MNEMONIC"
  echo
fi

if [ -z "${SELLER_MNEMONIC+set}" ] || [ -z "$SELLER_MNEMONIC" ]; then
  SELLER_MNEMONIC=$(provenanced keys mnemonic)
  echo "- Missing SELLER_MNEMONIC; using:"
  echo
  echo "    $SELLER_MNEMONIC"
  echo
fi

if [ -z "${BASE_DENOM+set}" ] || [ -z "$BASE_DENOM" ]; then
  BASE_DENOM="gme.local"
  echo "- Missing BASE_DENOM; using BASE_DENOM=$BASE_DENOM"
fi

if [ -z "${BASE_AMOUNT+set}" ] || [ -z "$BASE_AMOUNT" ]; then
  BASE_AMOUNT=10000
  echo "- Missing BASE_AMOUNT; using BASE_AMOUNT=$BASE_AMOUNT"
fi

if [ -z "${QUOTE_DENOM+set}" ] || [ -z "$QUOTE_DENOM" ]; then
  QUOTE_DENOM="usd.local"
  echo "- Missing QUOTE_DENOM; using QUOTE_DENOM=$QUOTE_DENOM"
fi

if [ -z "${QUOTE_AMOUNT+set}" ] || [ -z "$QUOTE_AMOUNT" ]; then
  QUOTE_AMOUNT=10000
  echo "- Missing QUOTE_AMOUNT; using QUOTE_AMOUNT=$QUOTE_AMOUNT"
fi

###############################################################################
# Create a new marker
#
# Arguments:
# - denom : str => Marker Denomination
# - amount : number => Marker amount
###############################################################################
function create_marker {
  local denom="$1"
  local amount="$2"

  result=$(provenanced tx marker new "${amount}${denom}" \
    --type COIN \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes)
  echo "- Created marker ${amount}${denom}"

  results=$(provenanced tx marker grant "$node0" "$denom" mint,burn,admin,withdraw,deposit \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes)
  echo "- Granted marker $denom => mint,burn,admin,withdraw,deposit"

  results=$(provenanced tx marker finalize "$denom" \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes)
  echo "- Finalized marker $denom"

  results=$(provenanced tx marker activate "$denom" \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes)
  echo "- Activated marker $denom"
}

###############################################################################
# Import a key into the provenanced key store if it does not exist
#
# Arguments:
# - key_name : str => The name of the key to import
# - key_mnemonic : str => The key mnemonic
###############################################################################
function import_key() {
  local key_name="$1"
  local key_mnemonic="$2"
  provenanced keys delete "$key_name" --keyring-backend test --testnet --yes &> /dev/null || true
  provenanced keys add "$key_name" \
    --keyring-backend test \
    --recover \
    --testnet \
    --hd-path "$DERIVATION_PATH" <<< "$key_mnemonic"
}

###############################################################################
# Fund accounts
#
# Arguments:
# - node0 : str =>
# - address : str =>
# - denom : str =>
# - amount : number =>
###############################################################################
function fund_account() {
  local node0="$1"
  local address="$2"
  local denom="$3"
  local amount="$4"
  local hash_amount="100000000000000nhash"

  # Fund $address with hash:
  result=$(provenanced tx bank send "$node0" "$address" "$hash_amount" \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes \
    --output json)
  echo "Funded $address with $hash_amount"
  echo "$result" | jq "{height: .height, gas_used: .gas_used}"

  # Fund $address with denom:
  result=$(provenanced tx marker withdraw "$denom" "${amount}${denom}" "$address" \
    --from "$node0" \
    --home "$PIO_NODE" \
    --keyring-backend test \
    --chain-id "$CHAIN_ID" \
    --gas auto \
    --gas-prices 1905nhash \
    --gas-adjustment 2 \
    --testnet \
    --yes)
  echo "Funded $address with ${amount}${denom}"
}

###############################################################################

# Import buyer and seller keys:
echo "- Importing buyer key: \"$buyer_key_name\""
import_key "$buyer_key_name" "$BUYER_MNEMONIC"

echo "- Importing seller key: \"$seller_key_name\""
import_key "$seller_key_name" "$SELLER_MNEMONIC"

# Get the buyer and seller addresses:
buyer=$(provenanced keys show -a "$buyer_key_name" --home "$PIO_HOME" --keyring-backend test --testnet)
echo "- Buyer address = $buyer"

seller=$(provenanced keys show -a "$seller_key_name" --home "$PIO_HOME" --keyring-backend test --testnet)
echo "- Seller address = $seller"

# Create markers:
echo "- Creating ${BASE_AMOUNT}${BASE_DENOM}"
create_marker "$BASE_DENOM" "$BASE_AMOUNT"

echo "- Creating ${QUOTE_AMOUNT}${QUOTE_DENOM}"
create_marker "$QUOTE_DENOM" "$QUOTE_AMOUNT"

# Fun buyer and seller accounts:
echo "- Funding buyer account $node0 => $buyer with ${QUOTE_AMOUNT}${QUOTE_DENOM}"
fund_account "$node0" "$buyer" "$QUOTE_DENOM" "$QUOTE_AMOUNT"

echo "- Funding seller account $node0 => $seller with ${BASE_AMOUNT}${BASE_DENOM}"
fund_account "$node0" "$seller" "$BASE_DENOM" "$BASE_AMOUNT"

# Bind name "sc.pb" -- this is the namespace used by the simulator + elsewhere
# when an orderbook is created, e.g. "<baseDenom>-ex.sc.pb"
result=$(provenanced tx name bind \
    "sc" \
    "$node0" \
    "pb" \
    --unrestrict \
    --from "$node0" \
    --keyring-backend test \
    --home "$PIO_NODE" \
    --chain-id "$CHAIN_ID" \
    --gas-prices 1905nhash \
	  --gas-adjustment=1.5 \
    --yes \
    --testnet \
    --output json)
echo
echo "- Bound name \"sc.pb\" (unrestricted)"
