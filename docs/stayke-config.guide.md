# Guía `stayke-config` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Parámetros globales compartidos y vault de comisiones de plataforma.

## Camino rápido

1. Admin llama una vez `initialize_config(minimum_deposit, fee_bps)` → `GlobalConfig` + platform vault.
2. Otros programas **leen** `GlobalConfig` (mint, fees, vault, `minimum_deposit`).
3. Hoy **no** hay instrucción de withdraw de fees; **no** hay program IDs de Core/Escrow/Disputes/Treasury en la cuenta.

## Detalles

### Instrucciones

| Instrucción | Estado |
|-------------|--------|
| `initialize_config` | Implementada |
| Withdraw fees desde `platform_vault` | TODO en `lib.rs` — no existe |

### `GlobalConfig` (campos actuales)

| Campo | Uso |
|-------|-----|
| `authority` | Admin |
| `minimum_deposit` | Umbral leído por treasury/escrow |
| `fee_bps` | Comisión en liquidaciones |
| `usdc_mint` | Mint autorizado |
| `platform_vault` / bump | Destino de fees |
| `is_initialized`, `bump` | Guardas PDA |

**Ausente:** Pubkeys de los programas Stayke. El TODO en `state.rs` pide registrarlos para validar firmantes CPI sin hardcodear.

### Objetivo SoT / seguridad vs estado actual

| Objetivo (diseño CPI) | As implemented |
|-----------------------|----------------|
| `GlobalConfig` como registro de program IDs para CPIs seguros | **Incompleto** — solo params económicos + vault |
| Withdraw de fees por authority | **Pendiente** |

Esto afecta cómo se endurecen mutadores en Core (ver [security](./stayke-todos-security.guide.md)): no afirmar que GlobalConfig ya es “single source of truth” completa para identidades de programas.

### Policy SoT vs On-chain gate

`minimum_deposit` en config es el número que Escrow/Treasury usan como gate. La política de cuándo ese depósito es obligatorio u opcional vive en [ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md) (L1–L6), no en este programa. Ver callouts en [escrow](./stayke-escrow.guide.md) y [architecture-flow](./stayke-architecture-flow.guide.md).

## Gaps

- GlobalConfig incompleto (sin program IDs).
- Sin withdraw fees.
- Valor de `minimum_deposit` vs política de bond opcional = gap de producto↔código (otros changes).

## Checklist

- [ ] No afirmé que GlobalConfig ya guarda program IDs
- [ ] Sé que withdraw fees no existe
- [ ] Enlacé el umbral a los callouts de escrow/treasury
