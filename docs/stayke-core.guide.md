# Guía `stayke-core` (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Estado de usuarios, reputación, identity y listings. Otros programas leen o mutan vía CPI.

## Camino rápido

1. Admin: `initialize_config` → `ConfigAcc`.
2. Usuario: `initialize_user_profile` → `UserProfile` + `ReputationProfile` (sin Identity aún).
3. Authority: `init_identity` → PDA Identity; luego `link_identity` enlaza al perfil.
4. Host: `initialize_listing`. Mutadores CPI: `update_deposit`, `clear_active_booking`, `add_infraction`, `clear_listing_booking`.

## Detalles

### Instrucciones expuestas (`lib.rs`)

| Instrucción | Quién | Efecto |
|-------------|-------|--------|
| `initialize_config` | Admin | Crea `ConfigAcc` (authority) |
| `initialize_user_profile` | Usuario | Crea `UserProfile` + `ReputationProfile` |
| `initialize_listing` | Host | Crea `Listing` |
| `init_identity` | Authority de Core | Crea `Identity` (`verified_at`, `linked=false`) |
| `link_identity` | Authority de Core | Enlaza Identity → `UserProfile.identity` |
| `update_deposit` | CPI / caller | ± `UserProfile.deposited` |
| `clear_active_booking` | CPI / caller | Limpia booking activo del perfil |
| `add_infraction` | CPI / caller | Incrementa contadores en `ReputationProfile` |
| `clear_listing_booking` | CPI / caller | `Listing.is_occupied = None` |

**No existen** en el programa actual: `verify_identity`, `set_host_status`. La “verificación” operativa para escrow es `UserProfile.identity.is_some()` tras `link_identity`.

### Cuentas clave

| Cuenta | Seeds (resumen) | Campos útiles |
|--------|-----------------|---------------|
| `UserProfile` | `user_profile` + authority | `identity`, `deposited`, `banned`, `listings` |
| `ReputationProfile` | `reputation_profile` + authority | reviews, infractions |
| `Identity` | id hash + `identity` | `verified_at`, `linked` |
| `Listing` | `listing` + owner + listing_id | `price`, `is_occupied` |

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

- Mutadores CPI aún con TODOs de autorización dura (ver [security](./stayke-todos-security.guide.md)).
- Handlers de update listing pueden existir en módulos; **solo** documentamos lo expuesto en `lib.rs` como API pública del programa.
- Política KYC de producto (Didit, etc.) → SoT / ADR-008; on-chain solo `init_identity` + `link_identity`.

## Checklist

- [ ] Usé `init_identity` / `link_identity`, no APIs inventadas
- [ ] No documenté `verify_identity` ni `set_host_status` como existentes
- [ ] Sé que escrow mira `identity` + `deposited`, no un flag `is_verified` legacy
