# Arquitectura on-chain — flujo CPI (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Vista corta de los cinco programas y cómo se invocan entre sí. La norma de producto y el System Design viven en stayke-docs; esta guía no los replica.

## Camino rápido

1. Config parametriza mint, fees, `minimum_deposit` y la **allowlist de program IDs**; los demás programas **leen** `GlobalConfig` y autorizan sus CPIs contra ella.
2. Core guarda perfiles, identity y listings; Treasury actualiza `deposited` vía CPI.
3. Escrow corre el lifecycle del booking; Disputes congela/reparte escrow y, por separado, puede penalizar treasury+reputación.
4. Detalle de cuentas/CPI → [stayke-global.guide.md](./stayke-global.guide.md); diagramas → [stayke-flow-diagram.md](./stayke-flow-diagram.md); seguridad → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md).

## Detalles

### Roles (ángulo on-chain)

| Programa | Rol as implemented |
|----------|-------------------|
| `stayke-config` | `GlobalConfig` (4 program IDs + `platform_vault` + `usdc_mint` + `free_ops`) + platform vault; `withdraw_fees` |
| `stayke-core` | `UserProfile`, `ReputationProfile`, `Identity`, `Listing`; 13 mutadores CPI con `assert_cpi_authority` |
| `stayke-treasury` | Depósito/retiro de garantía; endpoint CPI `cpi_penalize_transfer` |
| `stayke-escrow` | Booking + vault por reserva; CPIs de disputa y de settlement/reputación hacia core |
| `stayke-disputes` | Abrir/resolver/cerrar disputa; `penalize_user` aparte (flujo admin-céntrico, ver Gaps) |

### CPI relevantes (resumen)

| Origen → destino | Instrucción / CPI | Cuándo |
|------------------|-------------------|--------|
| Treasury → Core | `update_deposit` | `deposit_guarantee` / `withdraw_guarantee` |
| Escrow → Core | `update_deposit`, `set_active_booking`, `clear_active_booking`, `set_listing_occupied`, `clear_listing_booking` | lifecycle del booking |
| Escrow → Core | `update_host_review`, `update_client_review`, `update_listing_review`, `increment_completed_stays`, `increment_hosted_stays`, `increment_client_cancellations`, `increment_host_cancellations` | settlement / cancelaciones |
| Disputes → Escrow | `cpi_update_booking_status` | `open_dispute` (→ `Disputed`) |
| Disputes → Escrow | `cpi_resolve_dispute_transfer` | `resolve_dispute` (split + fee + cierre escrow; exige `status == Disputed`) |
| Disputes → Treasury | `cpi_penalize_transfer` | `penalize_user` (no en `resolve`) |
| Disputes → Core | `update_deposit`, `add_infraction` | `penalize_user` |
| Disputes → Core | `clear_active_booking` (guest+host), `clear_listing_booking` | `close_dispute` |

Toda la autorización CPI corre por el PDA `cpi_authority` de cada programa contra la allowlist de `GlobalConfig` (`AllowedCaller`: Treasury/Escrow/Disputes/Core).

### Policy SoT vs On-chain gate — bond / `minimum_deposit` / identity / free tier

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond ≠ escrow ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)); L1/L4 permiten host/guest sin bond en Stage 1 ([ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md)). |
| **On-chain** | `create_booking` exige `UserProfile.deposited >= GlobalConfig.minimum_deposit` para **guest y host**, `identity.is_some()`, y consume `free_ops` cuando aplica. |
| **Gap** | El gate on-chain trata el depósito de treasury como prerequisito de reserva; la política SoT no exige bond para listar/reservar en Stage 1. Alineación de código → otros changes MVP. |

## Gaps

- **Disputas admin-céntricas (principal):** `resolve_dispute`, `penalize_user` y `close_dispute` dependen de un único admin (sin quórum ni rangos); no hay resolución P2P ni ventana de disputa. Plan → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md) («Próxima iteración»).
- `resolve_dispute` no llama a treasury ni reputación; la penalización económica es instrucción separada (`penalize_user`).
- Yield/lending → diferido SoT ([ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)); stubs en treasury no están expuestos en `lib.rs`.

## Checklist

- [ ] No usé esta guía como norma de producto
- [ ] Sé que resolve ≠ penalize
- [ ] Conocí el gap de `minimum_deposit` vs L1/L4
- [ ] `GlobalConfig` ya registra los 4 program IDs (no lo marco como incompleto)