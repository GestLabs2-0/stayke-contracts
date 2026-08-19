# Referencia técnica global — stayke-contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Program IDs, cuentas/seeds y mapa CPI. Complementa las guías por programa; no sustituye ECONOMIC-MODEL ni ADRs.

## Camino rápido

1. Busca el program ID / cuenta en las tablas.
2. Verifica CPI en el mapa (especialmente Disputes → Treasury vía `penalize_user` y Escrow → Treasury vía `host_cancel_booking`).
3. Si el tema es política de bond/yield → SoT, no esta página.

## Detalles

### Program IDs

| Programa | ID |
|---|---|
| `stayke-config` | `9ESE5Ztpr8zWbLyXCyiB5QqcjxHghotT8zqJxD2S3zaT` |
| `stayke-core` | `2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm` |
| `stayke-escrow` | `68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a` |
| `stayke-disputes` | `8vgDvWkdqhpGBPAczpmZ3DJahVgNN36soRnyw6MbfMCJ` |
| `stayke-treasury` | `HV16vUTaZ78bJP1CyH5KDWyx8NqS1MYSGdPkRsMcnSuY` |

### Cuentas (resumen)

#### stayke-config — `GlobalConfig` (seed `global_config`)

`authority`, `minimum_deposit`, `fee_bps`, `usdc_mint`, `is_initialized`, `free_ops`,
`platform_vault`, `platform_vault_bump`, `core_program`, `escrow_program`,
`disputes_program`, `treasury_program`, `bump`.

> Ahora almacena los program IDs de los otros cuatro programas — es la SoT de la CPI
> allowlist (`assert_cpi_authority` en core la consulta). El TODO anterior está resuelto.

#### stayke-core

| Cuenta | Seeds | Notas |
|--------|-------|-------|
| `ConfigAcc` | `config` | Authority Core |
| `UserProfile` | `user_profile` + authority | `identity`, `active_booking`, `deposited`, `lending`, `staked`, `banned`, `listings`, `hosted_stays`, `completed_stays` |
| `ReputationProfile` | `reputation_profile` + authority | Reviews (host/client), cancellations, infractions (low/medium/high), skipped reviews |
| `Identity` | `identity` | `verified_at`, `linked` |
| `Listing` | `listing` + owner + listing_id | `listing_id`, `price`, `rating`, `total_reviews`, `is_active`, `is_occupied`, `state_hash`, `content_ref` |

Identity on-chain: `init_identity` / `link_identity` (no `verify_identity`).

#### stayke-escrow

| Cuenta | Seeds | Notas |
|--------|-------|-------|
| `EscrowConfig` | `escrow_config` | `authority`, `is_initialized`, `bump` |
| `Booking` | `booking` + property + guest + check_in | State machine completa (ver lifecycle abajo) |
| `BookingDays` | `booking_days` + property + year | `occupied_days: [u32; 12]` (bitmask mensual), `year`, `bump` |

`BookingStatus`: `Pending`, `HostAccepted`, `Active`, `Completed`, `Released`, `Cancelled`,
`Disputed`, `DisputeResolved`, `DisputeRejected`.

#### stayke-disputes

| Cuenta | Seeds | Notas |
|--------|-------|-------|
| `DisputeConfig` | — | `admins: Vec<Pubkey>`, `retribution_bps_low/medium/high`, `is_initialized`, `bump` |
| `Dispute` | por booking | `booking`, `property`, `initiator`, `guilty`, `reason`, `status`, `created_at`, `resolved_at` |

#### stayke-treasury

| Cuenta | Seeds | Notas |
|--------|-------|-------|
| `TreasuryConfig` | `treasury_config` | `authority`, `treasury_vault`, `treasury_bump`, `global_config`, `is_initialized`, `bump` |

Lending/stake no expuestos en `lib.rs` (placeholders).

### Mapa CPI (código actual)

```
Treasury
  deposit_guarantee / withdraw_guarantee
    ──CPI──> Core::update_deposit

Escrow::booking_starts
    ──CPI──> Core::set_active_booking (guest)

Escrow::release_funds
    ──CPI──> Core::increment_completed_stays (guest)
    ──CPI──> Core::clear_active_booking (guest)
    ──CPI──> Core::increment_hosted_stays (host)

Escrow::guest_cancel_booking
    ──CPI──> Core::increment_client_cancellations (guest reputation)

Escrow::host_cancel_booking
    ──CPI──> Treasury::cpi_penalize_transfer (slash al guest)
    ──CPI──> Core::update_deposit (decrement host)
    ──CPI──> Core::increment_host_cancellations (host reputation)

Escrow::guest_review
    ──CPI──> Core::update_host_review
    ──CPI──> Core::update_listing_review

Escrow::host_review
    ──CPI──> Core::update_client_review

Disputes::open_dispute
    ──CPI──> Escrow::cpi_update_booking_status(Disputed)

Disputes::resolve_dispute
    ──CPI──> Escrow::cpi_resolve_dispute_transfer
    (no Treasury, no reputación)

Disputes::penalize_user
    ──CPI──> Treasury::cpi_penalize_transfer
    ──CPI──> Core::update_deposit (resta)
    ──CPI──> Core::add_infraction

Disputes::close_dispute
    ──CPI──> Core::clear_active_booking (guest/host)
    ──CPI──> Core::clear_listing_booking
```

Lecturas frecuentes: Escrow → `UserProfile` / `Listing` / `GlobalConfig`; Treasury → `GlobalConfig` + `UserProfile`.

### Booking lifecycle

```
create_booking / create_booking_cross_year → Pending
  ├─ host_accept_booking → HostAccepted
  │    ├─ booking_starts (*permissionless*, check-in reached) → Active
  │    │    ├─ booking_completes (*permissionless*, check-out reached) → Completed
  │    │    │    └─ release_funds (*permissionless*, +24 h) → Released
  │    │    │         fee → platform_vault · host_amount → host · escrow cerrado
  │    │    └─ open_dispute (CPI) → Disputed → resolve → DisputeResolved|Rejected
  │    ├─ guest_cancel_booking → Cancelled (split 60/75/remainder o full refund)
  │    └─ host_cancel_booking → Cancelled (full refund + slash 10% si ≤72 h)
  ├─ host_reject_booking → Cancelled (full refund)
  └─ expire_booking (*permissionless*, +24 h sin respuesta) → Cancelled (full refund)

Reviews post-settlement (Completed | Released | DisputeResolved | DisputeRejected):
  guest_review(1-5) → host reputation + listing rating
  host_review(1-5)  → guest reputation
```

### Policy SoT vs On-chain gate — `minimum_deposit` (free tier)

Escrow exige depósito de forma **condicional** (free tier): mientras
`(completed_stays + hosted_stays) < free_ops`, no se exige depósito; al superar el umbral,
`deposited >= minimum_deposit` es requerido. Aplica a guest y host en `create_booking`,
`create_booking_cross_year` y `host_accept_booking`. SoT L1/L4: bond opcional Stage 1.
Detalle: [escrow](./stayke-escrow.guide.md), [free-tier](./stayke-free-tier.guide.md),
[architecture-flow](./stayke-architecture-flow.guide.md).

### Yield

Placeholders / stubs → Stage 2+ ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)). No documentar como feature live.

## Gaps

1. Withdraw fees pendiente en config.
2. Gate deposit vs política bond opcional — resuelto parcialmente con free tier (`free_ops`).
3. `penalize_user` desacoplado de `resolve_dispute` (por diseño actual).
4. `host_reject_booking_cross_year` existe; no hay `create_booking` cross-year test en escrow.
5. TODOs de seguridad CPI → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md).

## Checklist

- [ ] CPI Disputes→Treasury aparece vía `penalize_user`, no como TODO fantasma
- [ ] `resolve` no mezcla treasury
- [ ] GlobalConfig almacena program IDs (CPI allowlist SoT)
- [ ] Escrow→Core CPIs documentados (release_funds, cancellations, reviews)
- [ ] Escrow→Treasury CPI documentado (host_cancel_booking)
- [ ] Yield apunta a ADR-010