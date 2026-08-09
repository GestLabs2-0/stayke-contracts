#!/usr/bin/env bash

set -euo pipefail

RPC_URL="http://localhost:8899"

OWNER_KEYPAIR="${1:?Owner keypair requerido}"
DECIMALS="${2:-6}"
MINT_AMOUNT="${3:-1000000}"

MINT_ADDRESS=$(
    spl-token create-token \
        --url "$RPC_URL" \
        --decimals "$DECIMALS" \
    | awk '/Creating token/ {print $3}'
)

TOKEN_ACCOUNT=$(
    spl-token create-account \
        "$MINT_ADDRESS" \
        --owner "$OWNER_KEYPAIR" \
        --url "$RPC_URL" \
    | awk '/Creating account/ {print $3}'
)

spl-token mint \
    "$MINT_ADDRESS" \
    "$MINT_AMOUNT" \
    "$TOKEN_ACCOUNT" \
    --url "$RPC_URL"

echo "Mint Address: $MINT_ADDRESS"
echo "Token Account Address: $TOKEN_ACCOUNT"
