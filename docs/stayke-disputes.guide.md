# Guía `stayke-disputes` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Disputa del booking con **resolución P2P primero**: quien abre (guest u host) tiene una ventana de 24 h para retirar/cerrar la disputa; si no, se escala a admin (`DisputeConfig.admins`), que resuelve con un `DisputeOutcome` — split del escrow + penalización de depósito + infracción de reputación en **un solo paso**. La penalización ya **no** es una instrucción aparte (`penalize_user` fue eliminado; ver Gaps).

## Camino rápido

1. Guest o host (identity + no banned, booking `Active` o `Completed`): `open_dispute` → la disputa nace `OpenP2P` y el booking pasa a `Disputed` (CPI Escrow `cpi_update_booking_status`).
2. Ventana P2P (24 h desde `opened_at`): el opener puede retirar con `solve_dispute_before_admin` → restaura el estado original del booking y el flujo normal (`release_funds`) continúa.
3. Pasada la ventana (permissionless): `escalate_dispute` → `Escalated`. Guest u host (opener o contraparte) pueden enlazar evidencia: `link_evidence(evidence: [u8;32])`.
4. Admin: `resolve_dispute(outcome)` → escrow (`cpi_resolve_dispute_transfer`) + treasury (`cpi_penalize_transfer`) + Core (`add_infraction`, `update_deposit`). `NoFaultFound` no mueve fondos.
5. Permissionless: `close_dispute` → contabiliza stays (ruta admin con escrow ya drenado) o restaura el booking (`NoFaultFound`); estado `Closed`, rent devuelta al opener.

## Detalles

### Estado y configuración

- `DisputeAccount` (PDA `[dispute, booking.key()]`): `opened_by` (`DisputeParty::Guest|Host`), `state` (`DisputeState`), `opened_at`, `guest_evidence`/`host_evidence` (`Option<[u8; 32]>`), `outcome` (`Option<DisputeOutcome>`), `original_booking_status` (BookingStatus), bump.
- `DisputeConfig` (PDA `dispute_config`): `admins` (Vec, `MAX_ADMINS = 3`), `retribution_bps_low/medium/high = 1000/3000/10000`, `is_initialized`, bump. **`retribution_bps_*` se guardan pero `resolve_dispute` no los usa** (ver Gaps).

### Máquina de estados

```
OpenP2P ──solve_dispute_before_admin (solo opener, ≤ 24 h)──> ResolvedByP2P
OpenP2P ──escalate_dispute (permissionless, > 24 h)──────────> Escalated ──resolve_dispute (admin)──> ResolvedByAdmin
ResolvedByP2P | ResolvedByAdmin ──close_dispute (permissionless)──> Closed
```

### Instrucciones

| Instrucción | Quién | Efecto principal |
|-------------|-------|------------------|
| `initialize_config` | Authority | Crea `DisputeConfig`: agrega el authority a `admins` y fija `retribution_bps_*`. Sin instrucciones add/remove admin |
| `open_dispute` | Guest o host del booking | Crea `Dispute` `OpenP2P`; guarda `original_booking_status`; CPI Escrow → `Disputed` |
| `solve_dispute_before_admin` | Solo el opener, dentro de la ventana (≤ 24 h) | `ResolvedByP2P`; restaura `original_booking_status` (descongela el booking) |
| `escalate_dispute` | **Permissionless** (solo avanza la máquina) | Requiere `now > opened_at + 24 h`; `Escalated` |
| `link_evidence(evidence)` | **Guest u host del booking** (opener o contraparte) | Solo en `Escalated`; fija `guest_evidence` o `host_evidence` |
| `resolve_dispute(outcome)` | Admin (`config.admins`) | Juicio único: escrow + treasury + reputación; → `ResolvedByAdmin` |
| `close_dispute` | **Permissionless** (disputa resuelta) | Contabiliza stays (ruta admin) o restaura booking (`NoFaultFound`); → `Closed`; rent al opener |

### `open_dispute`

- `initiator_profile`: `!banned` y `identity.is_some()`.
- Initiator debe ser `booking.guest` o `booking.host` (`UnauthorizedDisputeInitiator`); `opened_by` se deriva de ahí.
- `booking.status` debe ser `Active` o `Completed` (`BookingNotDisputable`).
- **No hay ventana de tiempo**: un booking `Completed` puede disputarse sin límite (gap, ver abajo).
- Guarda `original_booking_status`; CPI Escrow firma con PDA `cpi_authority` → `BookingStatus::Disputed`. No existe `guilty` al abrir: culpable/víctima los decide el admin con `outcome`.

### Ventana P2P y escalado

- `DISPUTE_P2P_WINDOW_SECONDS = 86_400` (24 h), medida desde `opened_at`.
- `solve_dispute_before_admin`: solo el opener (`UnauthorizedDisputeSolver`), solo con `now <= opened_at + 24 h` (`P2PWindowElapsed` en caso contrario). Restaura el booking a `original_booking_status` vía CPI, así `release_funds` corre el flujo feliz y cuenta los stays.
- `escalate_dispute`: sin firmante exigido (permite a cualquiera avanzar el estado); requiere `now > opened_at + 24 h` (`EscalationWindowNotElapsed`).
- `link_evidence`: solo en `Escalated` (`DisputeNotEscalated`); exige `!banned` + `identity.is_some()`. **Cualquiera de las dos partes del booking puede enlazar** (guest → `guest_evidence`, host → `host_evidence`; cualquier otro firmante → `UnauthorizedUser`). El error `EvidenceLinked` está definido pero no se enforcea (sobrescribe la evidencia previa). Gap.

### `resolve_dispute(outcome)` — penalización integrada (sin `penalize_user`)

El admin resuelve con `DisputeOutcome`; `find_judgement` mapea culpable/víctima:

| Outcome | Culpable | Víctima | Escrow (víctima del distributable) | Depósito del culpable (treasury → víctima) |
|---------|----------|---------|--------------------------------------|---------------------------------------------|
| `GuestFavored{sev}` | host | guest | Low 0 % / Med 50 % / High 100 % | Low 0 / Med 30 % / High 100 % |
| `HostFavored{sev}` | guest | host | **100 % siempre** (el host como víctima) | Low 0 / Med 30 % / High 100 % |
| `NoFaultFound` | — | — | Sin transferencias | Sin transferencias |
| `MaliciousClaim{sev}` | **el opener** | contraparte | Según quién abrió (mismo mapa) | Ídem |

- Slashes por constantes compile-time (`ESCROW_SLASH_MEDIUM_BPS = 5_000`, `ESCROW_SLASH_HIGH_BPS = 10_000`, `DEPOSIT_SLASH_MEDIUM_BPS = 3_000`, `DEPOSIT_SLASH_HIGH_BPS = 10_000`); **no** vienen de `DisputeConfig.retribution_bps_*` (gap).
- Con culpable, en orden: CPI Core `add_infraction(guilty_reputation, severity)` → CPI Escrow `cpi_resolve_dispute_transfer(escrow_slash)` → CPI Treasury `cpi_penalize_transfer(victim_amount)` (victim_amount = `deposited − slash`) → CPI Core `update_deposit(guilty, slash, false)`.
- `NoFaultFound` → no hay Judgement: **ningún CPI de fondos**; el booking queda `Disputed` hasta `close_dispute`.
- Exige `dispute.state == Escalated` (`DisputeNotEscalated`) y admin en `config.admins` (`UnauthorizedAdmin`). Finaliza en `ResolvedByAdmin`. Nada produce `DisputeRejected` hoy.

### `close_dispute` (permissionless)

- Requiere `ResolvedByAdmin` o `ResolvedByP2P` (`DisputeNotResolved`). El `opener_wallet` pasado debe ser la wallet de quien abrió (`InvalidOpenerWallet`); la rent de la cuenta `Dispute` vuelve al opener.
- Si `ResolvedByAdmin` y `booking.status == Disputed` (ruta `NoFaultFound`): CPI Escrow restaura `original_booking_status` → `release_funds` reparte y cuenta los stays.
- Si `ResolvedByAdmin` y `booking.status == DisputeResolved` (escrow ya drenado): aquí se liquidan los contadores — `increment_completed_stays` (guest), `clear_active_booking` (guest), `increment_hosted_stays` (host) — porque `release_funds` ya no correrá para ese booking.
- Estado → `Closed`; cierra la cuenta `Dispute`.

### Notas de CPI (escrow)

- `cpi_update_booking_status` exige `assert_cpi_authority([AllowedCaller::Disputes])`; valida `Active|Completed → Disputed` y `Disputed → DisputeResolved | DisputeRejected`; cualquier otro valor se asigna sin validar transición (por eso `solve`/`close` pueden restaurar `original_booking_status`).
- `DisputeRejected` queda en el enum de `BookingStatus`, pero **ninguna ruta lo produce** hoy: la resolución siempre termina en `DisputeResolved`.

## Gaps

- **~~Flujo admin-céntrico sin P2P~~** → **resuelto**: hay ventana P2P (24 h), retiro del opener (`solve_dispute_before_admin`) y escalado permissionless; la penalización ya no es instrucción aparte (`penalize_user` eliminado, vive en `resolve_dispute`).
- **Admin único y sin quórum (escalado):** `DisputeConfig.admins` solo carga al authority del init (no existen instrucciones add/remove admin) y `resolve_dispute` depende de ese único admin, sin quórum ni rangos.
- **Sin ventana de apertura:** `open_dispute` acepta `Active` o `Completed` sin límite temporal (un booking `Completed` puede disputarse indefinidamente).
- ~~**Evidencia solo del opener**~~ → **resuelto**: `link_evidence` acepta al guest o al host del booking (opener o contraparte), solo tras escalar. Sigue abierto: `EvidenceLinked` sin enforce (sobrescritura de evidencia).
- **`retribution_bps_*` muertos:** se guardan en `DisputeConfig` pero el juicio usa constantes compile-time.
- **Intermediarios (roadmap):** `open_dispute` exige initiator = `booking.guest` o `booking.host`; si terceros pasan a intermediar bookings, hay que definir qué rol abre la disputa y en representación de quién. Plan → [stayke-todos-security.guide.md](./stayke-todos-security.guide.md) («Próxima iteración: terceros como intermediarios»).

### Policy SoT vs On-chain gate (contexto)

Slash de bond (tesorería) vs liquidación de escrow son instrumentos distintos ([ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md)). On-chain los ejecuta en **dos CPIs separadas dentro de `resolve_dispute`** (escrow split vs treasury penalize); la política de montos/severidad vive en SoT / OPEN-QUESTIONS, no aquí.

## Checklist

- [ ] Sé que la penalización ya no es instrucción aparte (`penalize_user` eliminado; vive en `resolve_dispute`)
- [ ] Conozco la ventana P2P de 24 h, el retiro del opener y el escalado permissionless
- [ ] Conozco la tabla `DisputeOutcome` → culpable/víctima y slashes
- [ ] Sé que `NoFaultFound` no mueve fondos y que `close_dispute` restaura el booking o liquida los stays
- [ ] Documenté los gaps vivos (admin único, sin ventana de apertura, `EvidenceLinked` sin enforce, `retribution_bps_*` sin uso)
- [ ] Documenté que los intermediarios en bookings son roadmap (rol en disputa sin definir)
