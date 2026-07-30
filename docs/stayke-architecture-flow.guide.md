# Arquitectura on-chain — flujo CPI (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Vista corta de los cinco programas y cómo se invocan entre sí. La norma de producto y el System Design viven en stayke-docs; esta guía no los replica.

## Camino rápido

1. Config parametriza mint, fees y `minimum_deposit`; los demás programas **leen** `GlobalConfig`.
2. Core guarda perfiles, identity y listings; Treasury actualiza `deposited` vía CPI.
3. Escrow corre el lifecycle del booking; Disputes congela/reparte escrow y, por separado, puede penalizar treasury+reputación.
4. Detalle de cuentas/CPI → [stayke-global.guide.md](./stayke-global.guide.md); diagramas → [stayke-flow-diagram.md](./stayke-flow-diagram.md).

## Detalles

### Roles (ángulo on-chain)

| Programa | Rol as implemented |
|----------|-------------------|
| `stayke-config` | `GlobalConfig` + platform vault; **sin** program IDs registrados aún |
| `stayke-core` | `UserProfile`, `ReputationProfile`, `Identity`, `Listing` |
| `stayke-treasury` | Depósito/retiro de garantía; endpoint CPI `cpi_penalize_transfer` |
| `stayke-escrow` | Booking + vault por reserva; CPIs de disputa |
| `stayke-disputes` | Abrir/resolver/cerrar disputa; `penalize_user` aparte |

### CPI relevantes (resumen)

| Origen → destino | Instrucción / CPI | Cuándo |
|------------------|-------------------|--------|
| Treasury → Core | `update_deposit` | `deposit_guarantee` / `withdraw_guarantee` |
| Disputes → Escrow | `cpi_update_booking_status` | `open_dispute` |
| Disputes → Escrow | `cpi_resolve_dispute_transfer` | `resolve_dispute` (solo escrow) |
| Disputes → Treasury | `cpi_penalize_transfer` | `penalize_user` (no en `resolve`) |
| Disputes → Core | `update_deposit`, `add_infraction` | `penalize_user` |
| Disputes → Core | `clear_active_booking`, `clear_listing_booking` | `close_dispute` |

### Policy SoT vs On-chain gate — bond / `minimum_deposit`

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond ≠ escrow ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)); L1/L4 permiten host/guest sin bond en Stage 1 ([ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md)). |
| **On-chain** | `create_booking` exige `UserProfile.deposited >= GlobalConfig.minimum_deposit` para **guest y host**. |
| **Gap** | El gate on-chain trata el depósito de treasury como prerequisito de reserva; la política SoT no exige bond para listar/reservar en Stage 1. Alineación de código → otros changes MVP. |

## Gaps

- `GlobalConfig` incompleto (sin Pubkeys de programas) → validación CPI vía program IDs aún no centralizada.
- `resolve_dispute` no llama a treasury ni reputación; la penalización económica es instrucción separada (`penalize_user`).
- Yield/lending → diferido SoT ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)); stubs en treasury no están expuestos en `lib.rs`.

## Checklist

- [ ] No usé esta guía como norma de producto
- [ ] Sé que resolve ≠ penalize
- [ ] Conocí el gap de `minimum_deposit` vs L1/L4
