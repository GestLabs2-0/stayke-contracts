# TODOs y seguridad — Stayke contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Inventario de TODOs de seguridad/autorización y lógica pendiente alineado al código. Índice: [README.md](./README.md).

## Camino rápido

1. ~~Prioriza brechas CPI / firmantes~~ → **cubierto**: allowlist en `GlobalConfig` + `assert_cpi_authority` en los 19 callers.
2. ~~`GlobalConfig` incompleto~~ → **cubierto**: `GlobalConfig` ya persiste los 4 program IDs + `platform_vault` + `usdc_mint`.
3. **Próximo foco:** endurecer el flujo de disputas (poder admin → resolución P2P), sección crítica abajo.
4. Gaps de política bond vs gate → guías escrow/config, no “fix” de producto aquí.

## Detalles

### Brechas críticas (CPI / autorización)

#### 1. ~~Mutadores Core sin enforcement CPI duro~~ → CUBIERTO

- ~~**Ubicación:** `stayke-core` mutators / listing clear (TODOs «enforce security»).~~
- ~~**Riesgo:** Mutaciones de `deposited`, booking flags, listing occupancy pueden no exigir firmante = programa autorizado vía `GlobalConfig`.~~
- **Estado:** ✅ Los 13 mutadores CPI de core exigen `assert_cpi_authority` contra la allowlist de `GlobalConfig` (ver [core](./stayke-core.guide.md) y [config](./stayke-config.guide.md)).

#### 2. ~~Token accounts en `resolve_dispute`~~ → CUBIERTO

- ~~**Ubicación:** `stayke-disputes` `resolve_dispute.rs` (TODO validaciones mint/vault).~~
- **Estado:** ✅ `resolve_dispute` valida `mint == global_config.usdc_mint`, `platform_vault_token_account.key() == global_config.platform_vault`, y owner/mint de las token accounts de host y guest.

#### 3. ~~Admin signer vs PDA en disputes~~ → CUBIERTO

- ~~**Ubicación:** `close_dispute` / flujo disputes (TODO usar PDA del programa).~~
- **Estado:** ✅ Todos los CPIs de disputes se firman con el PDA `cpi_authority` (`CPI_AUTHORITY_SEED`), no con la wallet admin. El admin firma la transacción de entrada, pero on-chain el poder de mutar lo tiene el PDA del programa.

### Próxima iteración: endurecer el flujo de disputas (gap abierto, P2P)

**Problema actual (verificado en `stayke-disputes`):** el flujo es admin-céntrico y no permite resolución P2P. El admin tiene poder unilateral sobre fondos y reputación:

- `resolve_dispute(host_share_bps, rejected)` → **admin signer** elige arbitrariamente `host_share_bps` (0–10_000) y `rejected`; si `rejected=true` el host recibe TODO el distributable y el guest pierde todo. El fee de la plataforma se cobra siempre.
- `penalize_user(severity)` → **admin signer** decide severidad (Low 10% / Med 30% / High 100% de `deposited`), elige al afectado y mueve fondos del treasury vault sin participación de la contraparte.
- `close_dispute` → **admin signer** libera bookings y listings tras la resolución.
- `open_dispute` es la única pieza P2P (guest/host del booking, requiere identity + no banned), pero no tiene ventana de tiempo: un booking `Completed` puede disputarse indefinidamente.
- `Dispute.guilty` se fija al abrir (contraparte del initiator) pero **no** restringe quién puede ser penalizado ni cómo se resuelve.

**Dirección de endurecimiento propuesta:**

1. **Resolución P2P**: split aceptado por ambas partes (guest y host firman el acuerdo) con time-lock de apelación; sin aceptación bilateral → fallback determinista (p. ej. 50/50) o arbitraje.
2. **Límites al poder admin**:
   - `host_share_bps` dentro de un rango parametrizado en `DisputeConfig` (p. ej. 20–80%) salvo doble firma de admins.
   - Quórum (2-de-3 admins) para `resolve_dispute` / `penalize_user` con decisión irreversible (idempotente, una sola vez).
   - `penalize_user` restringido al `Dispute.guilty` de una disputa resuelta (no a un usuario arbitrario).
3. **Ventana de disputa**: solo booking `Active` o dentro de N días tras `Completed`; estado `Disputed` con caducidad.
4. **Separación de poderes**: la decisión (P2P/arbitraje) nunca debe poder mutar el escrow sin pasar por el CPI de escrow ya endurecido (`cpi_resolve_dispute_transfer` + `assert_cpi_authority`), que ya exige `booking.status == Disputed` y valida mint/vault.

### Desarrollo pendiente (no afirmar como hecho)

| Área | Estado as implemented |
|------|----------------------|
| ~~Program IDs en `GlobalConfig`~~ | ~~TODO — incompleto~~ → ✅ registrados (`core/escrow/disputes/treasury`) |
| ~~Withdraw fees (`stayke-config`)~~ | ~~TODO — instrucción ausente~~ → ✅ `withdraw_fees` implementado |
| ~~Mutadores core sin enforcement~~ | ~~TODO~~ → ✅ allowlist `assert_cpi_authority` (19 callers) |
| `penalize_user` → `cpi_penalize_transfer` | **Implementado** (no listar como “falta cablear”) |
| Flujo de disputas P2P / límites admin | **Gap abierto** — ver sección «Próxima iteración» arriba |
| Host ban mid-settlement (escrow) | TODO de caso borde |
| Lending / staking (treasury) | Stubs; no en `#[program]`; SoT [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md) |

### Hechos corregidos vs docs antiguas

- Disputes **sí** llama `cpi_penalize_transfer` desde `penalize_user`.
- `resolve_dispute` **no** es esa llamada; sí llama `cpi_resolve_dispute_transfer` (split + fee + cierre del escrow) exigiendo `booking.status == Disputed`.
- Core identity: `init_identity` / `link_identity` únicamente en API pública.

## Gaps

- **Disputas admin-céntricas (principal):** `resolve_dispute`, `penalize_user` y `close_dispute` dependen de un único admin; sin quórum, sin rangos, sin aceptación P2P, sin ventana de disputa ni caducidad. → plan en «Próxima iteración».
- Política SoT de bond opcional vs gates `minimum_deposit` → ver callouts en escrow/architecture-flow (fuera del alcance de “cerrar” en este archivo).

## Checklist

- [ ] No afirmé GlobalConfig incompleto (ya tiene los 4 program IDs)
- [ ] No marqué `cpi_penalize_transfer` como TODO de cableado
- [ ] Documenté el gap de disputas como rediseño pendiente (P2P, límites admin, ventana de disputa)
- [ ] Enlacé esta guía desde el hub