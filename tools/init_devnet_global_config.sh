#!/bin/bash

CORE_ID=$(solana-keygen pubkey target/deploy/stayke_core-keypair.json)
ESCROW_ID=$(solana-keygen pubkey target/deploy/stayke_escrow-keypair.json)
TREASURY_ID=$(solana-keygen pubkey target/deploy/stayke_treasury-keypair.json)
DISPUTES_ID=$(solana-keygen pubkey target/deploy/stayke_disputes-keypair.json)
WALLET="../../keypair.json"
URL="https://solana-devnet.g.alchemy.com/v2/RRPxZ9enn2QeJ47Hzs89nngHgfXTYGC0"
USDC_ADDRESS="4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU"

echo "$CORE_ID"

pnpm --filter initialize-contracts start -- \
  --program stayke-config \
  --keypair $WALLET \
  --rpc-url $URL \
  --mint-address $USDC_ADDRESS \
  --fee-bps 500 \
  --minimum-deposit 100000 \
  --max-operations 3 \
  --core-program $CORE_ID \
  --escrow-program $ESCROW_ID \
  --disputes-program $DISPUTES_ID \
  --treasury-program $TREASURY_ID
