# Referencia técnica global — stayke-contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Program IDs, cuentas/seeds y mapa CPI. Complementa las guías por programa; no sustituye ECONOMIC-MODEL ni ADRs.

## Camino rápido

1. Busca el program ID / cuenta en las tablas.
2. Verifica CPI en el mapa (especialmente Disputes → Treasury vía `penalize_user`).
3. Si el tema es política de bond/yield → SoT, no esta página.

## Detalles

### Program IDs

| Programa | ID |
|---|---|
| `stayke-config` | `2GM2yLmDtz2Hyb8T5VBftERmiyJ5whKUmv6V4hBjNXMW` |
| `stayke-core` | `8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP` |
| `stayke-escrow` | `FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ` |
| `stayke-disputes` | `7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB` |
| `stayke-treasury` | `59buEPHFBK4h8LyLE2KtnV1kpaQTyjb82NWt5F9jSuHu` |

### Cuentas (resumen)

#### stayke-config — `GlobalConfig` (seed `global_config`)

`authority`, `minimum_deposit`, `fee_bps`, `usdc_mint`, `platform_vault`, bumps, `is_initialized`.

**Incompleto vs objetivo CPI:** no almacena program IDs de los otros programas (TODO en `state.rs`).

#### stayke-core

| Cuenta | Seeds | Notas |
|--------|-------|-------|
| `ConfigAcc` | `config` | Authority Core |
| `UserProfile` | `user_profile` + authority | `identity`, `deposited`, `banned`, `listings` |
| `ReputationProfile` | `reputation_profile` + authority | Scores e infracciones |
| `Identity` | id hash + `identity` | `verified_at`, `linked` |
| `Listing` | `listing` + owner + listing_id | `price`, `is_occupied` |

Identity on-chain: `init_identity` / `link_identity` (no `verify_identity`).

#### stayke-escrow

`EscrowConfig`, `Booking` (status machine), `BookingDays` (bitmask mensual).

#### stayke-disputes

`DisputeConfig` (admins, retribution bps), `Dispute` por booking.

#### stayke-treasury

`TreasuryConfig`, vault PDA. Lending/stake no expuestos en `lib.rs`.

### Mapa CPI (código actual)

```
Treasury
  deposit_guarantee / withdraw_guarantee
    ──CPI──> Core::update_deposit

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
create_booking → Pending
  host_accept → HostAccepted → client_accept → Active
  → review_completed → ReviewCompleted → complete_stay → Completed
  Active → open_dispute → Disputed → resolve → DisputeResolved|Rejected → close_dispute
```

### Policy SoT vs On-chain gate — `minimum_deposit`

Escrow exige `deposited >= minimum_deposit` (guest y host en `create_booking`). SoT L1/L4: bond opcional Stage 1. Detalle: [escrow](./stayke-escrow.guide.md), [architecture-flow](./stayke-architecture-flow.guide.md).

### Yield

Placeholders / stubs → Stage 2+ ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)). No documentar como feature live.

## Gaps

1. Program IDs no están en `GlobalConfig`.
2. Withdraw fees pendiente en config.
3. Gate deposit vs política bond opcional.
4. `penalize_user` desacoplado de `resolve_dispute` (por diseño actual).
5. TODOs de seguridad CPI → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md).

## Checklist

- [ ] CPI Disputes→Treasury aparece vía `penalize_user`, no como TODO fantasma
- [ ] `resolve` no mezcla treasury
- [ ] GlobalConfig marcado incompleto
- [ ] Yield apunta a ADR-010
