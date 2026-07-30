# TODOs y seguridad — Stayke contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Inventario de TODOs de seguridad/autorización y lógica pendiente alineado al código. Índice: [README.md](./README.md).

## Camino rápido

1. Prioriza brechas CPI / firmantes (sección crítica).
2. Recuerda: `GlobalConfig` **no** está completo (sin program IDs) — no asumas registro central de programas.
3. Gaps de política bond vs gate → guías escrow/config, no “fix” de producto aquí.

## Detalles

### Brechas críticas (CPI / autorización)

#### 1. Mutadores Core sin enforcement CPI duro

- **Ubicación:** `stayke-core` mutators / listing clear (TODOs «enforce security»).
- **Riesgo:** Mutaciones de `deposited`, booking flags, listing occupancy pueden no exigir firmante = programa autorizado vía `GlobalConfig`.
- **Nota:** El remedio deseado (program IDs en GlobalConfig) **aún no está** en `GlobalConfig` — ver [config](./stayke-config.guide.md).

#### 2. Token accounts en `resolve_dispute`

- **Ubicación:** `stayke-disputes` `resolve_dispute.rs` (TODO validaciones mint/vault).
- **Riesgo:** Cuentas de token incorrectas en la resolución del escrow del booking.

#### 3. Admin signer vs PDA en disputes

- **Ubicación:** `close_dispute` / flujo disputes (TODO usar PDA del programa).
- **Riesgo:** Dependencia de wallets admin para firmar CPIs.

### Desarrollo pendiente (no afirmar como hecho)

| Área | Estado as implemented |
|------|----------------------|
| Program IDs en `GlobalConfig` | TODO — **incompleto** |
| Withdraw fees (`stayke-config`) | TODO — instrucción ausente |
| `penalize_user` → `cpi_penalize_transfer` | **Implementado** (no listar como “falta cablear”) |
| Host ban mid-settlement (escrow) | TODO de caso borde |
| Lending / staking (treasury) | Stubs; no en `#[program]`; SoT [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md) |

### Hechos corregidos vs docs antiguas

- Disputes **sí** llama `cpi_penalize_transfer` desde `penalize_user`.
- `resolve_dispute` **no** es esa llamada.
- Core identity: `init_identity` / `link_identity` únicamente en API pública.

## Gaps

- Seguridad CPI acoplada a GlobalConfig completo → bloqueada hasta registrar program IDs.
- Política SoT de bond opcional vs gates `minimum_deposit` → ver callouts en escrow/architecture-flow (fuera del alcance de “cerrar” en este archivo).

## Checklist

- [ ] No afirmé GlobalConfig completo
- [ ] No marqué `cpi_penalize_transfer` como TODO de cableado
- [ ] Enlacé esta guía desde el hub
