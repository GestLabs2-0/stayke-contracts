#!/bin/bash
# =============================================================================
# update_programs.sh — Stayke: extend ProgramData + re-deploy + IDL upload
#
# Uso:
#   ./update_programs.sh                  # devnet (default)
#   ./update_programs.sh --net devnet
#   ./update_programs.sh --net localnet
#   ./update_programs.sh --net devnet --extend-only   # solo extiende, no deploya
#   ./update_programs.sh --net devnet --deploy-only   # asume ya extendido
#   ./update_programs.sh --net devnet --skip-idl      # deploy sin subir IDL
#
# Requisitos:
#   - anchor CLI instalado y en PATH
#   - solana CLI instalado y en PATH
#   - keypair.json en la raíz del workspace
#   - Programas ya buildeados (anchor build ejecutado previamente)
# =============================================================================

set -euo pipefail

# ─── Colores ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_info()    { echo -e "${CYAN}[INFO]${NC}  $*"; }
log_ok()      { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $*"; }
log_section() { echo -e "\n${BOLD}${CYAN}=== $* ===${NC}"; }

# ─── Defaults ─────────────────────────────────────────────────────────────────
NET="devnet"
CUSTOM_RPC_URL=""
EXTEND_ONLY=false
DEPLOY_ONLY=false
SKIP_IDL=false

# ─── Parse args ───────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --net)         NET="$2"; shift 2 ;;
    --rpc-url)     CUSTOM_RPC_URL="$2"; shift 2 ;;
    --extend-only) EXTEND_ONLY=true; shift ;;
    --deploy-only) DEPLOY_ONLY=true; shift ;;
    --skip-idl)    SKIP_IDL=true; shift ;;
    -h|--help)
      grep '^#' "$0" | head -20 | sed 's/^# \?//'
      exit 0 ;;
    *) log_error "Argumento desconocido: $1"; exit 1 ;;
  esac
done

# ─── Configuración por red ────────────────────────────────────────────────────
# --rpc-url tiene prioridad sobre --net si se pasan los dos
if [[ -n "$CUSTOM_RPC_URL" ]]; then
  CLUSTER="$CUSTOM_RPC_URL"
else
  case "$NET" in
    devnet)
      CLUSTER="https://solana-devnet.g.alchemy.com/v2/RRPxZ9enn2QeJ47Hzs89nngHgfXTYGC0"
      ;;
    localnet)
      CLUSTER="http://127.0.0.1:8899"
      ;;
    *)
      log_error "Red no soportada: '$NET'. Usa 'devnet' o 'localnet'."
      exit 1
      ;;
  esac
fi

WALLET="./keypair.json"

PROGRAMS=(
  "stayke_config"
  "stayke_core"
  "stayke_treasury"
  "stayke_escrow"
  "stayke_disputes"
)

# Bytes adicionales a agregar sobre el tamaño actual del .so
# 200 KB de margen es suficiente para la mayoría de iteraciones de desarrollo.
# Aumenta este valor si sigues recibiendo "account data too small".
EXTEND_BYTES=204800

# ─── Validaciones previas ─────────────────────────────────────────────────────
log_section "PRE-FLIGHT CHECKS (red: $NET)"

if [[ ! -f "$WALLET" ]]; then
  log_error "No se encontró el keypair en: $WALLET"
  exit 1
fi

for cmd in solana anchor solana-keygen; do
  if ! command -v "$cmd" &>/dev/null; then
    log_error "Comando no encontrado: $cmd"
    exit 1
  fi
done

log_ok "Wallet:  $WALLET"
log_ok "Cluster: $CLUSTER"
log_ok "Programas: ${PROGRAMS[*]}"

# ─── Verificar que los artefactos existen ─────────────────────────────────────
for program in "${PROGRAMS[@]}"; do
  SO_PATH="target/deploy/${program}.so"
  KP_PATH="target/deploy/${program}-keypair.json"
  if [[ ! -f "$SO_PATH" ]]; then
    log_error "Binario no encontrado: $SO_PATH — ejecuta 'anchor build' primero."
    exit 1
  fi
  if [[ ! -f "$KP_PATH" ]]; then
    log_error "Keypair no encontrado: $KP_PATH"
    exit 1
  fi
done
log_ok "Todos los artefactos de build encontrados."

# ─── PASO 1: Extender ProgramData accounts ────────────────────────────────────
if [[ "$DEPLOY_ONLY" == false ]]; then
  log_section "PASO 1 — EXTEND PROGRAM ACCOUNTS (+${EXTEND_BYTES} bytes)"

  for program in "${PROGRAMS[@]}"; do
    PROGRAM_ID=$(solana-keygen pubkey "target/deploy/${program}-keypair.json")
    SO_SIZE=$(wc -c < "target/deploy/${program}.so")

    log_info "$program ($PROGRAM_ID) — .so actual: ${SO_SIZE} bytes"

    # Verificar si el programa ya existe en la red
    if solana program show "$PROGRAM_ID" \
        --url "$CLUSTER" &>/dev/null 2>&1; then

      log_info "Extendiendo ProgramData en $NET..."
      solana program extend \
        "$PROGRAM_ID" \
        "$EXTEND_BYTES" \
        --url "$CLUSTER" \
        --keypair "$WALLET" \
        && log_ok "$program extendido correctamente." \
        || log_warn "$program: 'extend' falló (puede que ya tenga espacio suficiente, continuando...)"
    else
      log_warn "$program no existe aún en $NET — se creará en el paso de deploy."
    fi
  done
fi

[[ "$EXTEND_ONLY" == true ]] && { log_ok "Modo --extend-only: finalizado."; exit 0; }

# ─── PASO 2: Deploy de programas (sin IDL) ────────────────────────────────────
log_section "PASO 2 — DEPLOY PROGRAMS (sin IDL)"

# Cuántas veces reintentar el deploy completo si blockhash expira
MAX_DEPLOY_ATTEMPTS=5

# Bytes por chunk de escritura. El default de solana CLI es 1232.
# Bajar a 512 reduce la presión sobre el RPC y da más tiempo por blockhash.
WRITE_CHUNK_SIZE=512

deploy_program() {
  local program="$1"
  local PROGRAM_ID
  PROGRAM_ID=$(solana-keygen pubkey "target/deploy/${program}-keypair.json")
  local attempt=1

  while [[ $attempt -le $MAX_DEPLOY_ATTEMPTS ]]; do
    log_info "[$attempt/$MAX_DEPLOY_ATTEMPTS] Deploying $program ($PROGRAM_ID)..."

    # Usamos solana program deploy directamente para poder pasar
    # --max-sign-attempts y --with-compute-unit-price que anchor no expone.
    #
    # --max-sign-attempts 10        → reintenta la firma de cada chunk hasta 10 veces
    # --with-compute-unit-price 1   → prioriza las write txs en la cola del validator
    # --use-rpc                     → envía los chunks vía RPC en lugar de TPU directo
    #                                 (más lento pero más confiable en devnet/Helius)
    if solana program deploy \
        "target/deploy/${program}.so" \
        --program-id "target/deploy/${program}-keypair.json" \
        --keypair "$WALLET" \
        --url "$CLUSTER" \
        --max-sign-attempts 15 \
        --with-compute-unit-price 1 \
        --use-rpc; then

      log_ok "$program deployed exitosamente en el intento $attempt."
      return 0
    fi

    log_warn "$program falló en intento $attempt. Esperando 15s antes de reintentar..."
    sleep 15
    ((attempt++))
  done

  log_error "$program: todos los intentos fallaron. Abortando."
  return 1
}

for program in "${PROGRAMS[@]}"; do
  deploy_program "$program" || exit 1
done

# ─── PASO 3: Upload IDL ───────────────────────────────────────────────────────
if [[ "$SKIP_IDL" == false ]]; then
  log_section "PASO 3 — UPLOAD IDL"

  for program in "${PROGRAMS[@]}"; do
    PROGRAM_ID=$(solana-keygen pubkey "target/deploy/${program}-keypair.json")
    IDL_PATH="target/idl/${program}.json"

    if [[ ! -f "$IDL_PATH" ]]; then
      log_warn "IDL no encontrado para $program: $IDL_PATH — saltando."
      continue
    fi

    log_info "Procesando IDL de $program ($PROGRAM_ID)..."

    # Cerrar IDL account existente para liberar rent y evitar conflicto de tamaño
    anchor idl close \
      "$PROGRAM_ID" \
      --provider.cluster "$CLUSTER" \
      --provider.wallet "$WALLET" \
      && log_info "IDL account cerrado." \
      || log_warn "No había IDL account previo (normal en primer deploy)."

    # Init directo tras el close (más predecible que upgrade después de close)
    anchor idl init \
      --filepath "$IDL_PATH" \
      "$PROGRAM_ID" \
      --provider.cluster "$CLUSTER" \
      --provider.wallet "$WALLET" \
      && log_ok "IDL subido para $program." \
      || {
        log_warn "idl init falló, intentando upgrade..."
        anchor idl upgrade \
          --filepath "$IDL_PATH" \
          "$PROGRAM_ID" \
          --provider.cluster "$CLUSTER" \
          --provider.wallet "$WALLET" \
          && log_ok "IDL upgraded para $program." \
          || log_error "No se pudo subir IDL para $program. Verifica manualmente."
      }
  done
fi

# ─── RESUMEN ──────────────────────────────────────────────────────────────────
log_section "RESUMEN FINAL"

for program in "${PROGRAMS[@]}"; do
  PROGRAM_ID=$(solana-keygen pubkey "target/deploy/${program}-keypair.json" 2>/dev/null || echo "N/A")
  log_ok "$program → $PROGRAM_ID"
done

echo -e "\n${GREEN}${BOLD}✓ Pipeline completado en $NET.${NC}\n"
