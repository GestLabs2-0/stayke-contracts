# Guía `stayke-escrow` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Motor del booking: calendario, vault USDC por reserva y liquidación (feliz o vía disputa).

## Camino rápido

1. `initialize_escrow` enlaza a `GlobalConfig`.
2. Guest: `create_booking` (sin mover fondos aún) → host accept/reject → guest `client_accept_reserve` (USDC al vault).
3. Happy path: `review_completed` → `complete_stay` (host + fee a platform vault).
4. Disputa: Escrow solo reacciona a CPIs desde Disputes (`cpi_update_booking_status`, `cpi_resolve_dispute_transfer`).

## Detalles

### Instrucciones

| Instrucción | Rol |
|-------------|-----|
| `initialize_escrow` | Config del programa |
| `create_booking` | Guest crea `Pending`; gates de perfil/depósito |
| `host_accept_booking` / `host_reject_booking` | Host responde |
| `client_accept_reserve` / `client_reject_reserve` | Guest confirma (paga) o cancela |
| `review_completed` | Score 1–5; escribe reputación host |
| `complete_stay` | Distribuye escrow → host + `fee_bps` a platform |
| `cpi_update_booking_status` | CPI Disputes → congela booking |
| `cpi_resolve_dispute_transfer` | CPI Disputes → reparte vault y cierra |

### Policy SoT vs On-chain gate — `minimum_deposit`

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond de comportamiento opcional en Stage 1 para listar/reservar (L1, L4); escrow de reserva obligatorio al confirmar ([ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md), [ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)). |
| **On-chain** | En `create_booking`, guest y host deben cumplir `deposited >= global_config.minimum_deposit`. El mismo umbral reaparece en `host_accept_booking` y `client_accept_reserve` (guest). También se exige `identity.is_some()` y no banned. |
| **Gap** | El depósito en treasury actúa como gate duro de booking; la SoT no exige bond para esas acciones en Stage 1. **No** interpretar el gate como política SoT ya cumplida. Fix de código → fuera de este change. |

### Lifecycle (estados)

`Pending` → `HostAccepted` → `Active` → `ReviewCompleted` → `Completed`  
Ramas: `Cancelled`; `Disputed` → `DisputeResolved` | `DisputeRejected`.

Fondos de booking se mueven en `client_accept_reserve` (entrada) y `complete_stay` / `cpi_resolve_dispute_transfer` (salida). El bond/treasury es instrumento distinto (ADR-007).

## Gaps

- Gate `minimum_deposit` vs política L1/L4 (callout arriba).
- Caso borde host baneado mid-settlement: TODO en código (ver [security](./stayke-todos-security.guide.md)).

## Checklist

- [ ] Distingo escrow de booking vs depósito treasury
- [ ] Leí el callout Policy SoT vs On-chain gate
- [ ] Sé que disputa liquida vía CPI, no vía `complete_stay`
