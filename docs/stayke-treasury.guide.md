# Guía `stayke-treasury` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Custodia del depósito de garantía (bond on-chain) y endpoint CPI de penalización. Separado del escrow de booking ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)).

## Camino rápido

1. Admin: `initialize_treasury`.
2. Usuario: `deposit_guarantee` → USDC a vault + CPI Core `update_deposit(+)`.
3. Usuario: `withdraw_guarantee` → CPI Core `update_deposit(-)` + USDC de vuelta (bloqueado si hay booking activo o está banned).
4. Disputes/Escrow: `cpi_penalize_transfer` (transfer desde vault treasury).

## Detalles

### Instrucciones expuestas (`lib.rs`)

| Instrucción | Quién | Efecto / validaciones |
|-------------|-------|-----------------------|
| `initialize_treasury` | Authority | Crea `TreasuryConfig` + `treasury_pda` (`treasury`) + `treasury_vault` (token authority = treasury_pda); valida `usdc_mint == global_config.usdc_mint` |
| `deposit_guarantee(amount)` | Usuario | `amount >= global_config.minimum_deposit` (`DepositTooLow`); transfer user→vault; CPI Core `update_deposit(amount, true)` vía PDA `cpi_authority` |
| `withdraw_guarantee(amount)` | Usuario | `amount > 0`, `deposited >= amount`; CPI Core `update_deposit(amount, false)` (decremento **antes** del transfer); transfer vault→user firmado por `treasury_pda` |
| `cpi_penalize_transfer(amount)` | Disputes o Escrow | `assert_cpi_authority([Disputes, Escrow])`; `amount > 0`; transfer vault→destino firmado por `treasury_pda` |

**Lending / stake:** stubs en código; **no** publicados en el módulo `#[program]` actual. Yield Stage 1 → fuera de alcance SoT ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)).

### Cuenta `TreasuryConfig`

| Campo | Descripción |
|-------|-------------|
| `authority` | Admin que inicializó el treasury |
| `treasury_vault` | Token account (USDC) controlada por `treasury_pda` |
| `treasury_bump` | Bump del PDA `treasury` (firma CPIs) |
| `global_config` | Pubkey de `GlobalConfig` referenciada |
| `is_initialized` / `bump` | Guard de re-init + bump de la config |

### Gates extra en `withdraw_guarantee` (perfil Core)

- `user_profile.authority == signer`
- `!user_profile.banned` (`UserBanned`)
- `user_profile.active_booking.is_none()` (`ActiveBookingExists`) — no se puede retirar la garantía con reserva activa
- `user_profile.deposited >= amount` (`InsufficientBalance`)

### Relación con Escrow / booking

Treasury no abre bookings. Escrow lee `UserProfile.deposited` como gate (ver callout abajo). El caller permitido del endpoint de penalización es **Disputes y Escrow** (no solo Disputes).

### Policy SoT vs On-chain gate — depósito vs bond opcional

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond opcional según L1–L4; no bloquea listar/reservar Stage 1 por falta de bond. |
| **On-chain** | `deposit_guarantee` exige monto ≥ `minimum_deposit`; Escrow exige ese `deposited` para crear/aceptar reservas. |
| **Gap** | El depósito treasury funciona como gate de participación en reservas; la SoT lo trata como señal/tier opcional. No confundir con el escrow obligatorio del booking. |

## Gaps

- Yield/lending diferido (ADR-010); no habilitar stubs como feature Stage 1.
- El treasury es custodia pasiva: no hay aún límite por usuario (cap) ni escrow parcial contra garantías en disputa (relacionado con el rediseño de disputas P2P, ver [security](./stayke-todos-security.guide.md)).

## Checklist

- [ ] Distingo treasury (bond) de escrow (booking)
- [ ] Sé que `cpi_penalize_transfer` lo dispara `penalize_user` y permite caller Disputes + Escrow
- [ ] Documenté el gate de `withdraw_guarantee` (no active_booking, no banned)
- [ ] No documenté lend/stake como live