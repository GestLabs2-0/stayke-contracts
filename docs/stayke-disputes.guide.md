# Guía `stayke-disputes` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Arbitraje admin del booking: congelar escrow, repartir fondos de la reserva y — en instrucción aparte — penalizar depósito + reputación.

## Camino rápido

1. Guest/host: `open_dispute` → CPI Escrow `cpi_update_booking_status(Disputed)`.
2. Admin: `resolve_dispute` → **solo** CPI Escrow `cpi_resolve_dispute_transfer` (reparte vault del booking). **No** toca treasury ni reputación.
3. Admin (opcional, separada): `penalize_user` → CPI Treasury `cpi_penalize_transfer` + Core `update_deposit` + `add_infraction`.
4. Admin: `close_dispute` → CPI Core limpia perfiles/listing.

## Detalles

### Instrucciones

| Instrucción | Efecto principal |
|-------------|------------------|
| `initialize_config` | `DisputeConfig` (admins, `retribution_bps_*`) |
| `open_dispute` | Crea `Dispute`; congela booking vía Escrow |
| `resolve_dispute(host_share_bps, rejected)` | Solo escrow: reparte / rechaza |
| `penalize_user(severity)` | Treasury transfer + baja `deposited` + infracción |
| `close_dispute` | Cierra cuenta Dispute; limpia Core |

### `resolve_dispute` ≠ `penalize_user`

| | `resolve_dispute` | `penalize_user` |
|--|-------------------|-----------------|
| Escrow booking | Sí (`cpi_resolve_dispute_transfer`) | No |
| Treasury / bond | No | Sí (`cpi_penalize_transfer`) |
| Reputación | No | Sí (`add_infraction`) |
| Depósito Core | No | Sí (`update_deposit` restando) |

El endpoint `cpi_penalize_transfer` **sí está cableado** desde `penalize_user` (no es TODO pendiente de “conectar”). Lo pendiente de producto/flujo es cuándo el admin debe llamar `penalize_user` respecto al ciclo de disputa (comentario en código: aplicar en disputa abierta / antes de cerrar).

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

- Validaciones de token accounts en `resolve_dispute` incompletas (TODO).
- Firmantes admin en vez de PDA del programa (TODO).
- `penalize_user` no está acoplado automáticamente a `resolve_dispute`.

### Policy SoT vs On-chain gate (contexto)

Slash de bond (tesorería) vs liquidación de escrow son instrumentos distintos ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)). On-chain ya los separa en dos instrucciones; la política de montos/severidad vive en SoT / OPEN-QUESTIONS, no aquí.

## Checklist

- [ ] No asumí que `resolve` penaliza treasury
- [ ] Documenté `penalize_user` → `cpi_penalize_transfer` como implementado
- [ ] Sé el orden open → resolve → (penalize) → close
