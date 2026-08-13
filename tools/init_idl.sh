#!/bin/bash
# Sin set -e global, manejamos errores manualmente

URL="https://solana-devnet.g.alchemy.com/v2/RRPxZ9enn2QeJ47Hzs89nngHgfXTYGC0"
WALLET="./keypair.json"

PROGRAMS=(
  "stayke_config"
  "stayke_core"
  "stayke_treasury"
  "stayke_escrow"
  "stayke_disputes"
)

echo "=== UPLOAD IDLs ==="
for program in "${PROGRAMS[@]}"; do
  PROGRAM_ID=$(solana-keygen pubkey target/deploy/${program}-keypair.json)
  IDL_PATH="target/idl/${program}.json"

  echo "IDL para $program ($PROGRAM_ID)..."

  # close: ignoramos el exit code, verificamos on-chain
  anchor idl close \
    "$PROGRAM_ID" \
    --provider.cluster "$URL" \
    --provider.wallet "$WALLET" || true

  # Pausa para que el RPC confirme el estado
  sleep 3

  # init: ignoramos el exit code también
  anchor idl init \
    --filepath "$IDL_PATH" \
    "$PROGRAM_ID" \
    --provider.cluster "$URL" \
    --provider.wallet "$WALLET" || true

  sleep 3

  # Verificación real: fetch del IDL on-chain
  echo "  Verificando IDL on-chain..."
  if anchor idl fetch "$PROGRAM_ID" \
      --provider.cluster "$URL" > /tmp/idl_check_${program}.json 2>/dev/null; then
    echo "  ✓ IDL verificado para $program"
  else
    echo "  ✗ ADVERTENCIA: IDL no encontrado para $program — revisar manualmente"
  fi

done

echo "=== DONE ==="
