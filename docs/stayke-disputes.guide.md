# Guía `stayke-disputes` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Arbitraje del booking: congelar escrow, repartir fondos de la reserva y — en instrucción aparte — penalizar depósito + reputación. La decisión recae en admins (`DisputeConfig.admins`), sin mecanismo P2P aún (ver Gaps).

## Camino rápido

1. Guest/host (identity + no banned): `open_dispute` → CPI Escrow `cpi_update_booking_status(Disputed)`.
2. Admin: `resolve_dispute(host_share_bps, rejected)` → **solo** CPI Escrow `cpi_resolve_dispute_transfer` (split + fee + cierre del vault del booking). **No** toca treasury ni reputación.
3. Admin (opcional, separada): `penalize_user(severity)` → CPI Treasury `cpi_penalize_transfer` + Core `update_deposit` + `add_infraction`.
4. Admin: `close_dispute` → CPI Core limpia `active_booking` (guest+host) y `clear_listing_booking`; cierra la cuenta `Dispute`.

## Detalles

### Instrucciones

| Instrucción | Quién | Efecto principal |
|-------------|-------|------------------|
| `initialize_config` | Authority | Crea `DisputeConfig`: agrega el authority a `admins` (max 3) y fija `retribution_bps_low=1000`, `medium=3000`, `high=10000` |
| `open_dispute(reason)` | Guest o host del booking | Crea `Dispute` (`guilty` = contraparte); congela booking vía Escrow |
| `resolve_dispute(host_share_bps, rejected)` | Admin (`config.admins`) | Solo escrow: reparte o rechaza |
| `penalize_user(severity)` | Admin (`config.admins`) | Treasury transfer + baja `deposited` + infracción |
| `close_dispute` | Admin (`config.admins`) | Cierra cuenta `Dispute`; limpia Core |

### `open_dispute` (validaciones)

- `initiator_profile`: `!banned` y `identity.is_some()`.
- Initiator debe ser `booking.guest` o `booking.host` (`UnauthorizedDisputeInitiator`).
- `booking.status` debe ser `Active` o `Completed` (`BookingNotDisputable`).
- **No hay ventana de tiempo**: un booking `Completed` puede disputarse sin límite (gap, ver abajo).
- `Dispute` queda `Open`; `guilty` = contraparte del initiator. CPI Escrow firma con PDA `cpi_authority` → `BookingStatus::Disputed`.

### `resolve_dispute` ≠ `penalize_user`

| | `resolve_dispute` | `penalize_user` |
|--|-------------------|-----------------|
| Escrow booking | Sí (`cpi_resolve_dispute_transfer`) | No |
| Treasury / bond | No | Sí (`cpi_penalize_transfer`) |
| Reputación | No | Sí (`add_infraction`) |
| Depósito Core | No | Sí (`update_deposit` restando) |

- `resolve_dispute` exige `dispute.status == Open` y delega el split a Escrow (`cpi_resolve_dispute_transfer`), que valida `booking.status == Disputed`, mint/vault (`global_config.usdc_mint`/`platform_vault`) y owners de host/guest; cobra `fee_bps` siempre; cierra el escrow y pasa a `DisputeResolved` / `DisputeRejected`.
- `penalize_user`: retribution = `deposited × retribution_bps_{severity} / 10_000`, limitada a `deposited`; transfiere del vault treasury al `affected_wallet` y registra la infracción. El endpoint `cpi_penalize_transfer` **sí está cableado** (no es TODO pendiente de “conectar”).

### Flujo típico

```
open_dispute ──CPI──> Escrow status=Disputed
        │
        ▼
resolve_dispute ──CPI──> Escrow reparte vault booking
        │
        ├── (opcional) penalize_user ──CPI──> Treasury + Core
        │
        ▼
close_dispute ──CPI──> Core clear profiles / listing
```

## Gaps

- **Flujo admin-céntrico (principal):** `resolve_dispute`, `penalize_user` y `close_dispute` dependen de un único admin (sin quórum ni rangos en `host_share_bps`); no hay resolución P2P (aceptación bilateral), ventana de disputa ni caducidad del estado `Disputed`. Plan → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md).
- `penalize_user` no está acoplado automáticamente a `resolve_dispute` (sigue siendo decisión del admin, en instrucción aparte).
- `Dispute.guilty` se fija al abrir pero no restringe quién puede ser penalizado.

### Policy SoT vs On-chain gate (contexto)

Slash de bond (tesorería) vs liquidación de escrow son instrumentos distintos ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)). On-chain ya los separa en dos instrucciones; la política de montos/severidad vive en SoT / OPEN-QUESTIONS, no aquí.

## Checklist

- [ ] No asumí que `resolve` penaliza treasury
- [ ] Documenté `penalize_user` → `cpi_penalize_transfer` como implementado
- [ ] Sé el orden open → resolve → (penalize) → close
- [ ] Documenté el gap de disputas admin-céntricas / P2P pendiente