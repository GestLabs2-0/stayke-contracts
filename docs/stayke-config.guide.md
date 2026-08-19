# Guía `stayke-config` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Parámetros globales compartidos, vault de comisiones de plataforma y registro de program IDs (CPI allowlist).

## Camino rápido

1. Admin llama una vez `initialize_config(minimum_deposit, fee_bps, free_ops, allowed_programs)` → `GlobalConfig` + platform vault.
2. Otros programas **leen** `GlobalConfig` (mint, fees, vault, `minimum_deposit`, `free_ops`, program IDs).
3. `withdraw_fees(amount)` permite al authority drenar el `platform_vault`.

## Detalles

### Instrucciones

| Instrucción | Estado |
|-------------|--------|
| `initialize_config(minimum_deposit, fee_bps, free_ops, allowed_programs)` | Implementada |
| `withdraw_fees(amount)` | Implementada — solo `authority` |

### `initialize_config`

Crea `GlobalConfig` (PDA `["global_config"]`) y el token account del platform vault
(PDA `["platform_vault_token"]`, authority = PDA `["platform_vault"]`).

> **Detalle de seguridad:** el parámetro `allowed_programs` se **ignora** en el handler.
> Los program IDs persistidos en `GlobalConfig` vienen de constantes compiladas en el crate
> (`CORE_PROGRAM_ID`, `ESCROW_PROGRAM_ID`, `DISPUTES_PROGRAM_ID`, `TREASURY_PROGRAM_ID`).
> Un initializer no puede inyectar IDs propios — la CPI allowlist es SoT del binario, no del caller.

### `withdraw_fees(amount)`

Transfiere USDC desde `platform_vault` a una cuenta destino con mint autorizado
(`usdc_mint`). Solo el `authority` configurado puede invocarla. Valida `is_initialized`
y `amount > 0`.

### `GlobalConfig` (campos actuales)

| Campo | Uso |
|-------|-----|
| `authority` | Admin |
| `minimum_deposit` | Umbral leído por treasury/escrow |
| `fee_bps` | Comisión en liquidaciones (validada `< 10_000`) |
| `usdc_mint` | Mint autorizado |
| `free_ops` | Umbral free tier: operaciones gratis antes de exigir depósito |
| `platform_vault` | Destino de fees |
| `platform_vault_bump` | Bump del PDA signer del vault |
| `core_program` | Program ID de `stayke-core` (CPI allowlist SoT) |
| `escrow_program` | Program ID de `stayke-escrow` (CPI allowlist SoT) |
| `disputes_program` | Program ID de `stayke-disputes` (CPI allowlist SoT) |
| `treasury_program` | Program ID de `stayke-treasury` (CPI allowlist SoT) |
| `is_initialized`, `bump` | Guardas PDA |

### Objetivo SoT / seguridad vs estado actual

| Objetivo (diseño CPI) | As implemented |
|-----------------------|----------------|
| `GlobalConfig` como registro de program IDs para CPIs seguros | **Implementado** — 4 program IDs persistidos al init, desde constantes compiladas |
| Withdraw de fees por authority | **Implementado** — `withdraw_fees(amount)` |

`GlobalConfig` es ahora la "single source of truth" para identidades de programas: los
mutadores de Core validan el caller CPI contra esta allowlist (`assert_cpi_authority`),
y Escrow lee `fee_bps` / `usdc_mint` / `platform_vault` / `free_ops` para sus gates.

### Policy SoT vs On-chain gate

`minimum_deposit` en config es el número que Escrow/Treasury usan como gate, y `free_ops`
hace ese gate condicional (free tier). La política de cuándo ese depósito es obligatorio u
opcional vive en [ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md) (L1–L6), no en este programa. Ver callouts en [escrow](./stayke-escrow.guide.md), [free-tier](./stayke-free-tier.guide.md) y [architecture-flow](./stayke-architecture-flow.guide.md).

## Gaps

- Valor de `minimum_deposit` / `free_ops` vs política de bond opcional = gap de producto↔código (otros changes).

## Checklist

- [ ] GlobalConfig guarda los 4 program IDs (CPI allowlist SoT) — verificado en `state.rs`
- [ ] `withdraw_fees` existe y está documentado
- [ ] `allowed_programs` del caller se ignora — la allowlist viene de constantes compiladas
- [ ] Enlacé el umbral a los callouts de escrow/treasury/free-tier