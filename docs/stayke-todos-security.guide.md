# TODOs y seguridad — Stayke contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Inventario de TODOs de seguridad/autorización y lógica pendiente alineado al código. Índice: [README.md](./README.md).

## Camino rápido

1. ~~Prioriza brechas CPI / firmantes~~ → **cubierto**: allowlist en `GlobalConfig` + `assert_cpi_authority` en los 19 callers.
2. ~~`GlobalConfig` incompleto~~ → **cubierto**: `GlobalConfig` ya persiste los 4 program IDs + `platform_vault` + `usdc_mint`.
3. ~~**Próximo foco:** endurecer el flujo de disputas (poder admin → resolución P2P)~~ → **implementado**: flujo P2P-first (ventana 24 h, retiro del opener, escalado permissionless). Quedan los límites al admin (quórum/rangos) → guía [disputes](./stayke-disputes.guide.md).
4. Gaps de política bond vs gate → guías escrow/config, no “fix” de producto aquí.
5. **A futuro:** expandir el protocolo para terceros como **intermediarios** en bookings (roadmap) → sección «Próxima iteración» abajo.

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

### ~~Próxima iteración: endurecer el flujo de disputas (gap abierto, P2P)~~ → Flujo P2P-first implementado; límites al admin pendientes

**Estado (verificado en `stayke-disputes`, commit «feat: flow for disputes finished»):** el flujo admin-céntrico
fue reemplazado por un **flujo P2P-first ya implementado**:

- ~~`resolve_dispute(host_share_bps, rejected)` con split arbitrario del admin~~ → **resuelto**: `resolve_dispute(outcome)`
  deriva culpable/víctima y slashes por constantes (`ESCROW_SLASH_*`, `DEPOSIT_SLASH_*`).
- ~~`penalize_user(severity)` como instrucción aparte~~ → **eliminada**: la penalización (treasury + reputación +
  depósito) vive dentro de `resolve_dispute`.
- ~~`close_dispute` admin-céntrico~~ → **resuelto**: `close_dispute` es permissionless (contabiliza stays o
  restaura el booking); la rent vuelve al opener.
- ~~Sin resolución P2P~~ → **resuelto**: ventana `DISPUTE_P2P_WINDOW_SECONDS` (24 h) con `solve_dispute_before_admin`
  (retiro del opener) y `escalate_dispute` permissionless.
- **Sigue abierto:** `open_dispute` acepta `Active`/`Completed` sin ventana de tiempo; `DisputeConfig.admins` solo
  carga al authority del init (sin add/remove de admins ni quórum); ~~`link_evidence` solo del opener~~ (la contraparte
  también enlaza; `EvidenceLinked` definido pero no enforceado); `retribution_bps_*` se guardan pero no se usan
  en el juicio.

**Pendiente (límites al poder admin):**

1. **Quórum** (2-de-3 admins) para `resolve_dispute`, con decisión irreversible (idempotente, una sola vez).
2. **Gestión de admins** expuesta (add/remove) y rangos parametrizados en `DisputeConfig` (los `retribution_bps_*`
   hoy son decorativos).
3. **Ventana de apertura de disputa**: solo booking `Active` o dentro de N días tras `Completed`; caducidad del estado `Disputed`.
4. ~~**Evidencia bilateral**~~ → **resuelto**: `link_evidence` acepta a guest u host (opener o contraparte). Queda `EvidenceLinked` sin enforce (sobrescritura permitida).
5. **Separación de poderes** (ya cumplida on-chain): la decisión muta el escrow solo vía `cpi_resolve_dispute_transfer` + `assert_cpi_authority` (exige `booking.status == Disputed` y valida mint/vault).

### Desarrollo pendiente (no afirmar como hecho)

| Área | Estado as implemented |
|------|----------------------|
| ~~Program IDs en `GlobalConfig`~~ | ~~TODO — incompleto~~ → ✅ registrados (`core/escrow/disputes/treasury`) |
| ~~Withdraw fees (`stayke-config`)~~ | ~~TODO — instrucción ausente~~ → ✅ `withdraw_fees` implementado |
| ~~Mutadores core sin enforcement~~ | ~~TODO~~ → ✅ allowlist `assert_cpi_authority` (19 callers) |
| ~~`penalize_user` → `cpi_penalize_transfer`~~ | ~~Instrucción aparte~~ → **eliminada**: la penalización va dentro de `resolve_dispute(outcome)` |
| ~~Flujo de disputas P2P~~ / límites admin | P2P **implementado** (ventana 24 h + escalado); límites admin **siguen abiertos** — ver sección arriba |
| Host ban mid-settlement (escrow) | TODO de caso borde |
| Disponibilidad `BookingDays` por cliente (escrow) | **Gap de política/confianza** — ver sección «Próxima iteración» abajo |
| Lending / staking (treasury) | Stubs; no en `#[program]`; SoT [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md) |
| Terceros como intermediarios en bookings | **Roadmap** — ver sección «Próxima iteración» abajo |

### Próxima iteración: disponibilidad por cliente (`BookingDays` por guest)

**Observación (gap de confianza, sin impacto en fondos):** el bitmap `BookingDays` se deriva por **propiedad**
(`[BOOKING_DAYS_SEED, property, year]`), no por cliente. El único gate del guest en `create_booking` es
`client_profile.active_booking.is_none()`, y `active_booking` solo se fija en `booking_starts` (cuando la reserva
pasa a `Active`), no al crear en `Pending`. Consecuencia: un cliente puede mantener varias reservas `Pending`
solapadas en el tiempo en **propiedades distintas**, o reservar y no presentarse.

- **No es un bug de fondos:** no hay pérdida económica ni invariante del escrow roto; no autoriza doble reserva de
  un mismo inmueble (eso ya lo impide el bitmap por propiedad).
- **Es una decisión de producto/confianza:** el objetivo del protocolo es promover confianza; un `Pending`
  solapado o un no-show la reduce. Implementar `BookingDays` por guest (mismo esquema bitmap, keyed por
  `client_profile`) permitiría exigir que el cliente no reserve días ya ocupados por él mismo.
- **Cosas a revisar antes de implementar:** (1) permitir o no «hold options» (reservas `Pending` paralelas es
  comportamiento legítimo de exploración; exigir exclusividad rompería ese caso de uso); (2) `active_booking`
  debería fijarse ya en `create_booking` si se quiere exclusividad desde `Pending`, con su correspondiente
  liberación en cancel/reject/expire; (3) costo de renta de una cuenta adicional por guest/año.
- **Pendiente:** actualizar el protocolo para manejar los `bookingDays` de los clientes (decisión no tomada aún).

### Manejo de rent/fees al cerrar cuentas (Account Closing Fees / Rent Reclamation)

**Pregunta de diseño / TODO abierto:** ¿A quién debe reembolsarse la renta de las cuentas al cerrarse? ¿Debe almacenarse explícitamente el `payer` original en el estado de las cuentas (ej. `Booking.payer`)?

**Contexto actual (verificado en `programs/**`):**
- En `create_booking`: el `payer` (que puede ser el guest o un relayer) paga la renta de creación para `Booking`, `BookingDays` (si se inicializa) y `escrow_token_account`.
- En `host_reject_booking` y `expire_booking`: `booking` (`close = payer`) y `escrow_token_account` (`CloseAccount` a `payer`) devuelven la renta al `payer` que firma la transacción de cierre (no necesariamente quien pagó la creación).
- En `guest_cancel_booking` y `host_cancel_booking`: el `escrow_token_account` se cierra devolviendo la renta a `caller` (el guest o el host que cancela), mientras que la cuenta `Booking` no se cierra (queda en `Cancelled`).
- En `release_funds`: `escrow_token_account` se cierra devolviendo la renta a `payer` (caller permissionless que ejecuta el settlement tras la ventana de 24 h).
- En `cpi_resolve_dispute_transfer`: `escrow_token_account` se cierra enviando la renta a `platform_vault_token_account`.

**Trade-offs a evaluar:**
1. **Opción A — Guardar `payer: Pubkey` en `Booking`:**
   - *Pros:* Garantiza que los lamports de renta siempre regresen exactamente a quien financió la cuenta (ej. el guest), protegiendo contra la extracción de renta por parte de relayers o callers permissionless.
   - *Contras:* Agrega 32 bytes de espacio en la cuenta `Booking` (+ renta inicial). Requiere que el destino de cierre coincida con `booking.payer`.
2. **Opción B — Renta como incentivo al `payer` de la transacción de cierre (modelo crank / relayer):**
   - *Pros:* No incrementa el tamaño de `Booking`. Sirve como incentivo económico natural (o reembolso de gas) para que bots o relayers ejecuten transacciones permissionless (`release_funds`, `expire_booking`).
   - *Contras:* Si un tercero llama `expire_booking` o `release_funds`, absorbe los lamports de renta que originalmente puso el guest/relayer inicial.

- **Pendiente:** Definir la política económica de retorno de renta y decidir si se agrega `pub payer: Pubkey` en `Booking` (o si se estandariza el destino de renta hacia la plataforma o hacia el guest).

### Próxima iteración: terceros como intermediarios en bookings (roadmap)

**Objetivo (roadmap):** ampliar el protocolo para que terceros puedan actuar como **intermediarios** en los
bookings — agencias, property managers o corporate bookers que reservan y gestionan estancias en nombre de un
guest y/o un host, en lugar de ser siempre las partes directas del flujo actual.

**Contexto actual (verificado en `stayke-escrow`):** el flujo es estrictamente guest↔host directo:
`create_booking` lo firma y fondea el guest (transfiere el total al vault del booking), `host_accept_booking`
lo acepta el host y `release_funds` paga host + platform. No hay hoy noción de delegación ni de tercero
representando a una parte; `open_dispute` restringe el initiator a `booking.guest` o `booking.host`.

**Impacto esperado en el código (a decidir, no implementado):**

1. **Escrow:** definir quién firma y fondea el vault del booking cuando el guest real actúa a través de un
   intermediario, y quién recibe en `release_funds` (¿split directo o reparto del intermediario?).
2. **Autorización:** modelo de delegación (firma de la parte representada, allowlist/rol en
   `UserProfile`/`GlobalConfig`, o similar) que impida que el intermediario mueva fondos de terceros sin
   autorización explícita.
3. **Disputas:** quién puede abrir una disputa cuando media un tercero (guest/host real, intermediario o
   ambos) y cómo se asigna la responsabilidad (ver «Flujo de disputas P2P» arriba).
4. **Reputación/reviews:** a quién califican `host_review`/`guest_review` si la contraparte operativa es un
   intermediario.

**Cosas a revisar antes de implementar:** (1) caso de uso objetivo (¿solo booking en nombre del guest, o
también gestión del host/property manager?); (2) implicaciones de identidad/KYC (`identity.is_some()` se exige
hoy en ambos lados); (3) fees del intermediario (split on-chain vs fuera de protocolo); (4) compatibilidad con
el bitmap `BookingDays` por propiedad y coste de cuentas.

- **Pendiente:** decisión de producto/política no tomada; no implementado en `programs/**`.

### Hechos corregidos vs docs antiguas

- Disputes llama `cpi_penalize_transfer` desde `resolve_dispute` (la instrucción `penalize_user` ya no existe).
- `resolve_dispute` ejecuta ambos CPIs según `DisputeOutcome`: `cpi_resolve_dispute_transfer` (split + fee + cierre del escrow, exige `booking.status == Disputed`) y `cpi_penalize_transfer` (treasury), más Core `add_infraction` / `update_deposit`.
- Core identity: `init_identity` / `link_identity` únicamente en API pública.

## Gaps

- ~~**Disputas admin-céntricas (principal):** sin aceptación P2P, ventana o escalado~~ → **resuelto (P2P-first)**: ventana 24 h + retiro del opener + escalado permissionless + `close_dispute` permissionless. Queda **admin único sin quórum** (solo el authority del init), **sin ventana de apertura** y ~~**evidencia solo del opener**~~ → **resuelto**: `link_evidence` acepta a guest u host (opener o contraparte) → detalle en [stayke-disputes.guide.md](./stayke-disputes.guide.md).
- **Disponibilidad por cliente (confianza, sin impacto en fondos):** el bitmap `BookingDays` es solo por propiedad; el cliente puede mantener reservas `Pending` solapadas en distintas propiedades. Decisión de producto/política pendiente → sección «Próxima iteración».
- **Terceros como intermediarios en bookings (roadmap):** el flujo actual es guest↔host directo (`create_booking` firma el guest y fondea el escrow; `open_dispute` exige guest/host). Ampliar el protocolo para que terceros (agencias/property managers) intermedien bookings requiere decidir autorización, contraparte del escrow y roles en disputa → sección «Próxima iteración».
- Política SoT de bond opcional vs gates `minimum_deposit` → ver callouts en escrow/architecture-flow (fuera del alcance de “cerrar” en este archivo).

## Checklist

- [ ] No afirmé GlobalConfig incompleto (ya tiene los 4 program IDs)
- [ ] No marqué `cpi_penalize_transfer` como TODO de cableado
- [ ] Documenté que el flujo P2P de disputas ya está implementado y que quedan los límites al admin (quórum, ventana de apertura, evidencia)
- [ ] Documenté el roadmap de terceros (intermediarios) en bookings como pendiente, no implementado
- [ ] Enlacé esta guía desde el hub