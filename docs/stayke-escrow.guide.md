# Guía `stayke-escrow` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Motor del booking: calendario, vault USDC por reserva y liquidación (feliz o vía disputa).

## Camino rápido

1. `initialize_escrow` enlaza a `GlobalConfig`.
2. Guest: `create_booking` crea `Pending` **y fondea el escrow** (transfiere el precio total al vault del booking). Ya no existe un paso aparte de pago.
3. Host: `host_accept_booking` → `HostAccepted`, o `host_reject_booking` → `Cancelled` (refund al guest).
4. Transiciones permissionless por tiempo: `booking_starts` (`HostAccepted` → `Active` al llegar check-in) y `booking_completes` (`Active` → `Completed` al llegar check-out).
5. Happy path: `release_funds` (tras ventana de 24 h) → host + `fee_bps` a platform vault.
6. Reviews: `host_review` (host→guest) y `guest_review` (guest→host), permitidas en `Completed`/`Released`/`DisputeResolved`/`DisputeRejected`.
7. Disputa: Escrow solo reacciona a CPIs desde Disputes (`cpi_update_booking_status`, `cpi_resolve_dispute_transfer`).

## Detalles

### Instrucciones

| Instrucción | Rol |
|-------------|-----|
| `initialize_escrow` | Config del programa |
| `create_booking` / `create_booking_cross_year` | Guest crea `Pending`; gates de perfil/depósito/active_booking; **fondea el escrow** |
| `host_accept_booking` | Host acepta → `HostAccepted` |
| `host_reject_booking` / `host_reject_booking_cross_year` | Host rechaza → `Cancelled` + refund al guest |
| `booking_starts` | Permissionless: `HostAccepted` → `Active` al llegar check-in; CPI core `set_active_booking` |
| `booking_completes` | Permissionless: `Active` → `Completed` al llegar check-out |
| `release_funds` | Distribuye escrow → host + `fee_bps` a platform; CPIs core (`increment_completed_stays`, `clear_active_booking`, `increment_hosted_stays`) |
| `host_review` | Host califica guest (1–5); escribe `host_review` + reputación guest |
| `guest_review` | Guest califica host (1–5); escribe `guest_review` + reputación host |
| `guest_cancel_booking` / `guest_cancel_booking_cross_year` | Guest cancela; split de refund según ventana (ver constantes) |
| `host_cancel_booking` / `host_cancel_booking_cross_year` | Host cancela; full refund + posible slash de depósito |
| ~~`client_reject_reserve` / `client_reject_reserve_cross_year`~~ | ~~Guest rechaza reserva → `Cancelled`, libera días~~ → **instrucción eliminada**: no está en `lib.rs`; la cancelación del guest va por `guest_cancel_booking(_cross_year)` |
| `expire_booking` / `expire_booking_cross_year` | Expira `Pending` tras 24 h → refund al guest |
| `cpi_update_booking_status` | CPI Disputes → congela booking (`Disputed`), lo cierra (`DisputeResolved`) o restaura `Active`/`Completed` (retiro P2P / `NoFaultFound`) |
| `cpi_resolve_dispute_transfer` | CPI Disputes → drena vault: fee `fee_bps` siempre + split del distributable según `slash_bps` (% para la víctima, el culpable recibe el resto); cierra el vault (rent → `platform_vault`); booking → `DisputeResolved` |

### Policy SoT vs On-chain gate — `minimum_deposit`

| Capa | Qué dice |
|------|----------|
| **SoT** | Bond de comportamiento opcional en Stage 1 para listar/reservar (L1, L4); escrow de reserva obligatorio al confirmar ([ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md), [ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)). |
| **On-chain** | En `create_booking`, guest y host deben cumplir `(completed_stays + hosted_stays) < free_ops || deposited >= minimum_deposit` (bypass de free tier). El mismo gate reaparece en `host_accept_booking` (host). Además se exige `identity.is_some()`, no banned, y para el guest `active_booking.is_none()`. |
| **Gap** | El depósito en treasury actúa como gate de booking (con free tier); la SoT no exige bond para esas acciones en Stage 1. **No** interpretar el gate como política SoT ya cumplida. Fix de código → fuera de este change. |

### Política de cancelación (constantes compile-time)

`programs/stayke-escrow/src/constants.rs` — hardcodeadas para el MVP (a futuro se leerán del `Listing`/`UserProfile`):

| Constante | Valor | Uso |
|-----------|-------|-----|
| `CANCELLATION_WINDOW_HOURS` | 72 | Ventana (horas antes de check-in) que activa el split/penalización |
| `CANCELLATION_REFUND_PERCENTAGE` | 60 | % del `total_price` reembolsado al guest al cancelar dentro de ventana |
| `CANCELLATION_HOST_SHARE_PERCENTAGE` | 75 | % del remanente post-refund para el host (Stayke retiene el resto) |
| `HOST_CANCELLATION_PENALTY_PERCENTAGE` | 10 | % del depósito del host slasheado al cancelar dentro de ventana |

Fuera de la ventana: guest cancel → full refund (0 host, 0 fee); host cancel → full refund y **sin** slash.

### Lifecycle (estados)

`Pending` → `HostAccepted` → `Active` → `Completed` → `Released`  
Ramas: `Cancelled`; `Disputed` → `DisputeResolved` (split vía CPI) o → estado original (`Active`/`Completed`, retiro P2P o `NoFaultFound`). `DisputeRejected` queda en el enum pero **ninguna ruta lo produce hoy** (la resolución siempre termina en `DisputeResolved`).

- Fondos de booking entran en `create_booking` y salen en `release_funds`, `cpi_resolve_dispute_transfer`, `host_reject_booking`, `expire_booking` y los `*_cancel_booking`. El bond/treasury es instrumento distinto (ADR-007).
- `Released` marca que el escrow ya fue drenado y cerrado (terminal de fondos). Ya no existe el estado `ReviewCompleted`.
- `updated_at` se refresca en `host_accept_booking`, `booking_starts` y `booking_completes`; este último dispara las dos ventanas paralelas post-stay: 24 h para `release_funds` y 72 h para reviews.

## Gaps

- Gate `minimum_deposit` vs política L1/L4 (callout arriba).
- `host_accept_booking` tiene un chequeo de ventana tautológico (`booking.updated_at <= booking.updated_at + 24h` siempre true): no fuerza el accept dentro de 24 h.
- `host_reject_booking_cross_year` transfiere el refund pero **no** cierra el vault (`close_account` ausente, a diferencia de la variante single-year).
- `expire_booking` (ambas variantes) hace el refund pero **no** cierra el vault del escrow: la cuenta token queda abierta (rent bloqueada) tras drenar el saldo.
- Caso borde host baneado mid-settlement: TODO en código (ver [security](./stayke-todos-security.guide.md)).
- **Intermediarios en bookings (roadmap):** hoy `create_booking` y `host_accept_booking` son estrictamente guest↔host directos (firma/fondeo del guest; pago host+platform en `release_funds`). Permitir que terceros intermedien reservas requiere decidir quién firma, fondea y recibe, y el modelo de delegación → [security](./stayke-todos-security.guide.md) («Próxima iteración: terceros como intermediarios»).

## Checklist

- [ ] Distingo escrow de booking vs depósito treasury
- [ ] Leí el callout Policy SoT vs On-chain gate
- [ ] Sé que el escrow se fondea en `create_booking` (no hay `client_accept_reserve`)
- [ ] Sé que disputa liquida vía CPI, no vía `release_funds`
- [ ] Conozco las constantes de cancelación y dónde se aplican
- [ ] Sé que `client_reject_reserve` ya no existe (guest cancela vía `guest_cancel_booking`)
- [ ] Conozco el gap del vault sin cerrar en `expire_booking`
- [ ] Sé que los intermediarios en bookings son roadmap (flujo directo guest↔host hoy)
