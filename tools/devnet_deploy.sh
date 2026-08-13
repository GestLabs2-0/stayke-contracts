#!/bin/bash
set -e

CLUSTER="https://solana-devnet.g.alchemy.com/v2/RRPxZ9enn2QeJ47Hzs89nngHgfXTYGC0"
WALLET="./keypair.json"
URL="https://solana-devnet.g.alchemy.com/v2/RRPxZ9enn2QeJ47Hzs89nngHgfXTYGC0"

PROGRAMS=(
  "stayke_config"
  "stayke_core"
  "stayke_treasury"
  "stayke_escrow"
  "stayke_disputes"
)

# echo "=== BUILD ==="
# anchor build

echo "=== DEPLOY PROGRAMS (sin IDL) ==="
for program in "${PROGRAMS[@]}"; do
  echo "Deploying $program..."
  PROGRAM_ID=$(solana-keygen pubkey target/deploy/${program}-keypair.json)
  echo "Deploy para $program ($PROGRAM_ID)..."

  anchor program deploy \
    --program-name $program \
    --program-keypair target/deploy/${program}-keypair.json \
    --no-idl \
    --provider.cluster $URL \
    --provider.wallet $WALLET
done

echo "=== UPLOAD IDLs ==="
for program in "${PROGRAMS[@]}"; do
  PROGRAM_ID=$(solana-keygen pubkey target/deploy/${program}-keypair.json)
  IDL_PATH="target/idl/${program}.json"

  echo "IDL para $program ($PROGRAM_ID)..."

  anchor idl close \
       $PROGRAM_ID \
        --provider.cluster $URL \
        --provider.wallet $WALLET


  # Intenta upgrade primero, si falla hace init
  anchor idl upgrade \
    --filepath $IDL_PATH \
    $PROGRAM_ID \
    --provider.cluster $URL \
    --provider.wallet $WALLET \
  || anchor idl init \
    --filepath $IDL_PATH \
    $PROGRAM_ID \
    --provider.cluster $URL \
    --provider.wallet $WALLET
done

echo "=== DONE ==="
