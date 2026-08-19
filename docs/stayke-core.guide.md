# Guía `stayke-core` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Estado de usuarios, reputación, identity y listings. Otros programas leen o mutan vía CPI
(autorización dura por `assert_cpi_authority` contra la allowlist de `GlobalConfig`).

## Camino rápido

1. Admin: `initialize_config` → `ConfigAcc`.
2. Usuario: `initialize_user_profile` → `UserProfile` + `ReputationProfile` (sin Identity aún).
3. Authority: `init_identity` → PDA Identity; luego `link_identity` enlaza al perfil.
4. Host: `initialize_listing(price, listing_id, state_hash, content_ref)`; mutadores de listing propios (`update_listing_*`).
5. Mutadores CPI (reviews, cancellations, stays, bookings): solo vía allowlist `GlobalConfig`.

## Detalles

### Instrucciones expuestas (`lib.rs`)

**Admin / usuarios**

| Instrucción | Quién | Efecto |
|-------------|-------|--------|
| `initialize_config` | Admin | Crea `ConfigAcc` (authority) |
| `initialize_user_profile` | Usuario | Crea `UserProfile` + `ReputationProfile` |
| `initialize_listing(price, listing_id, state_hash, content_ref)` | Host | Crea `Listing` |

**Identity**

| Instrucción | Quién | Efecto |
|-------------|-------|--------|
| `init_identity(_id)` | Authority de Core | Crea `Identity` (`verified_at`, `linked=false`) |
| `link_identity(_id)` | Authority de Core | Enlaza Identity → `UserProfile.identity` |

**Mutadores de listing (propios del host)**

| Instrucción | Efecto |
|-------------|--------|
| `update_listing_state(state, content_ref)` | Actualiza `state_hash` / `content_ref` |
| `update_listing_price(price)` | Actualiza `price` |
| `update_listing_publish(active)` | Actualiza `is_active` |

**Mutadores CPI privilegiados** — requieren PDA `cpi_authority` + allowlist `GlobalConfig`:

| Instrucción | Caller permitido | Efecto |
|-------------|------------------|--------|
| `update_deposit(amount, is_deposit)` | Treasury, Disputes, Escrow | ± `UserProfile.deposited` |
| `clear_active_booking` | Disputes, Escrow | Limpia `active_booking` del perfil |
| `set_active_booking(booking)` | Escrow | Fija `active_booking` (booking_starts) |
| `add_infraction(severity)` | Disputes | Incrementa infractions en `ReputationProfile` |
| `clear_listing_booking` | Disputes, Escrow | Libera `Listing.is_occupied` |
| `set_listing_occupied(occupied)` | Escrow | Fija `Listing.is_occupied` |
| `update_host_review(score)` | Escrow | Acumula review del host (1–5) |
| `update_client_review(score)` | Escrow | Acumula review del client (1–5) |
| `update_listing_review(score, reviewer)` | Escrow | Acumula `total_reviews` / `rating` del listing |
| `increment_completed_stays` | Escrow | `UserProfile.completed_stays += 1` |
| `increment_hosted_stays` | Escrow | `UserProfile.hosted_stays += 1` |
| `increment_client_cancellations` | Escrow | `ReputationProfile.client_cancellations += 1` |
| `increment_host_cancellations` | Escrow | `ReputationProfile.host_cancellations += 1` |

La allowlist vive en `GlobalConfig` (`core_program`, `escrow_program`, `disputes_program`,
`treasury_program`) y se valida con `assert_cpi_authority(global_config, cpi_authority, allowed)`
(helper en `stayke-config::cpi_authority`). `AllowedCaller`: `Treasury | Escrow | Disputes | Core`.

**No existen** en el programa actual: `verify_identity`, `set_host_status`. La "verificación"
operativa para escrow es `UserProfile.identity.is_some()` tras `link_identity`.

### Cuentas clave

| Cuenta | Seeds (resumen) | Campos útiles |
|--------|-----------------|---------------|
| `UserProfile` | `user_profile` + authority | `identity`, `active_booking`, `deposited`, `lending`, `staked`, `banned`, `listings`, `hosted_stays`, `completed_stays` |
| `ReputationProfile` | `reputation_profile` + authority | `host_reviews`, `total_score_host`, `client_reviews`, `total_score_client`, `host_cancellations`, `client_cancellations`, `host_reviews_skipped`, `guest_reviews_skipped`, `low/medium/high_infractions` |
| `Identity` | `identity` | `verified_at`, `linked` |
| `Listing` | `listing` + owner + listing_id | `listing_id`, `price`, `rating`, `total_reviews`, `is_active`, `is_occupied`, `state_hash`, `content_ref` |

### Flujo identity (as implemented)

```
initialize_user_profile
        ↓
init_identity (authority)  →  Identity PDA
        ↓
link_identity (authority)  →  UserProfile.identity = Some(...)
        ↓
Escrow puede exigir identity.is_some() en create_booking
```

## Gaps

- Política KYC de producto (Didit, etc.) → SoT / ADR-008; on-chain solo `init_identity` + `link_identity`.
- `update_listing_*` como mutadores directos del host: no hay aún versión CPI-gated para que
  escrow/disputes actualicen listing state.
- TODOs de seguridad CPI restantes → [security](./stayke-todos-security.guide.md).

## Checklist

- [ ] Usé `init_identity` / `link_identity`, no APIs inventadas
- [ ] No documenté `verify_identity` ni `set_host_status` como existentes
- [ ] Documenté los 13 mutadores CPI con su caller permitido real (allowlist GlobalConfig)
- [ ] Documenté los 3 mutadores de listing propios del host
- [ ] Sé que escrow mira `identity` + `deposited` + `free_ops`, no un flag `is_verified` legacy