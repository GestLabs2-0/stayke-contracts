# Guía `stayke-treasury` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Custodia del depósito de garantía (bond on-chain) y endpoint CPI de penalización. Separado del escrow de booking ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)).

## Camino rápido

1. Admin: `initialize_treasury`.
2. Usuario: `deposit_guarantee` → USDC a vault + CPI Core `update_deposit(+)`
3. Usuario: `withdraw_guarantee` → CPI Core `update_deposit(-)` + USDC de vuelta (con checks de balance).
4. Disputes: `penalize_user` llama `cpi_penalize_transfer` (transfer desde vault treasury).

## Detalles

### Instrucciones expuestas (`lib.rs`)

| Instrucción | Efecto |
|-------------|--------|
| `initialize_treasury` | `TreasuryConfig` + vault |
| `deposit_guarantee(amount)` | Transfer user→vault; exige `amount >= minimum_deposit` de `GlobalConfig`; CPI Core |
| `withdraw_guarantee(amount)` | CPI Core decremento; transfer vault→user |
| `cpi_penalize_transfer(amount)` | CPI desde Disputes: mueve USDC del vault a destino |

**Lending / stake:** stubs en código; **no** publicados en el módulo `#[program]` actual. Yield Stage 1 → fuera de alcance SoT ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)).

### Relación con Escrow / booking

Treasury no abre bookings. Escrow lee `UserProfile.deposited` como gate (ver callout abajo).

### Policy SoT vs On-chain gate — depósito vs bond opcional

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond opcional según L1–L4; no bloquea listar/reservar Stage 1 por falta de bond. |
| **On-chain** | `deposit_guarantee` exige monto ≥ `minimum_deposit`; Escrow exige ese `deposited` para crear/aceptar reservas. |
| **Gap** | El depósito treasury funciona como gate de participación en reservas; la SoT lo trata como señal/tier opcional. No confundir con el escrow obligatorio del booking. |

## Gaps

- Yield/lending diferido (ADR-010); no habilitar stubs como feature Stage 1.
- Seguridad de mutadores Core al actualizar `deposited` (ver [security](./stayke-todos-security.guide.md)).

## Checklist

- [ ] Distingo treasury (bond) de escrow (booking)
- [ ] Sé que `cpi_penalize_transfer` lo dispara `penalize_user`, no `resolve_dispute`
- [ ] No documenté lend/stake como live
