# Guía Free Tier — Depósito condicional (STK-164)

> **As implemented** — documenta el código on-chain actual, no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md).

Mecanismo de depósito obligatorio condicional: los usuarios nuevos operan sin depósito
hasta que superan un umbral de operaciones, después del cual se les exige el depósito
configurado en `GlobalConfig.minimum_deposit`.

## Programas involucrados

| Programa | Rol |
|----------|-----|
| `stayke-config` | Almacena `free_ops` (umbral) y `minimum_deposit` (monto) en `GlobalConfig` |
| `stayke-core` | Mantiene contadores `completed_stays` y `hosted_stays` en `UserProfile` |
| `stayke-escrow` | Aplica la validación condicional en `create_booking` y `host_accept_booking`; incrementa contadores vía CPI en `release_funds` |

## Flujo de coordinación

```
┌──────────────────────────────────────────────────────────────────┐
│                       STAYKE-CONFIG                              │
│  GlobalConfig { free_ops, minimum_deposit }                      │
│  (PDA ["global_config"])                                         │
└────────────┬──────────────────────────────────┬─────────────────┘
             │ read                             │ read
             ▼                                  ▼
┌────────────────────────┐    ┌────────────────────────────────────┐
│     STAYKE-CORE         │    │        STAYKE-ESCROW                │
│ UserProfile {           │    │                                     │
│   completed_stays,      │◄───│ release_funds ──CPI──►              │
│   hosted_stays,         │    │   increment_completed_stays         │
│   deposited             │    │   clear_active_booking              │
│ }                       │    │   increment_hosted_stays            │
│                         │    │                                     │
│                         │    │ create_booking /                    │
│                         │    │ create_booking_cross_year /         │
│                         │    │ host_accept_booking                 │
│                         │    │   validación condicional:            │
│                         │    │   (completed+hosted) < free_ops      │
│                         │    │   || deposited >= minimum_deposit    │
└────────────────────────┘    └────────────────────────────────────┘
```

## Lógica de validación condicional

Cada instrucción del escrow que requiere depósito evalúa:

```
SI (user_profile.completed_stays + user_profile.hosted_stays) < global_config.free_ops
   → free tier activo: no se exige depósito
SINO
   → user_profile.deposited >= global_config.minimum_deposit requerido
```

Implementado como constraint de Anchor en los `#[account]` de cada instrucción:

```rust
// create_booking.rs — perfil del guest
constraint = (client_profile.completed_stays + client_profile.hosted_stays)
    < global_config.free_ops as u32
    || client_profile.deposited >= global_config.minimum_deposit
    @ EscrowError::InsufficientDeposit,
```

### Instrucciones con free tier gate

| Instrucción | Valida guest | Valida host | Notas |
|-------------|:-----------:|:-----------:|-------|
| `create_booking` | ✅ | ✅ | Ambos perfiles se verifican en la creación |
| `create_booking_cross_year` | ✅ | ✅ | Misma lógica, variant cross-year |
| `host_accept_booking` | — | ✅ | Re-check al aceptar por si cambió el estado |

> **Importante:** `booking_starts`, `booking_completes`, `release_funds`, `expire_booking`,
> `guest_cancel_booking`, `host_cancel_booking`, y las reviews **NO** tienen free tier gate
> — son transiciones de lifecycle que operan sobre bookings ya creados.

## CPI: incremento de contadores

Cuando un stay se completa, la secuencia de settlement es:

1. `booking_completes` — anyone, transitions `Active` → `Completed`. No mueve fondos.
2. `release_funds` — anyone, 24 h después de `booking_completes`. Distribuye fondos y ejecuta CPIs.

En `release_funds`, después de distribuir el host_amount y la fee:

```rust
// release_funds.rs — después de transfer_checked
let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];

// CPI 1: increment guest's completed_stays
stayke_core::cpi::increment_completed_stays(
    CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        UpdateUserProfile { user_profile, global_config, cpi_authority },
        signer_seeds,
    ),
)?;

// CPI 2: clear guest's active_booking slot
stayke_core::cpi::clear_active_booking(
    CpiContext::new_with_signer(/* ... */, signer_seeds),
)?;

// CPI 3: increment host's hosted_stays
stayke_core::cpi::increment_hosted_stays(
    CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        UpdateUserProfile { user_profile: host, global_config, cpi_authority },
        signer_seeds,
    ),
)?;
```

Los tres CPIs usan signer seeds `["cpi_authority"]` y son validados por
`assert_cpi_authority` en stayke-core contra la allowlist de `GlobalConfig`.

## Contadores existentes

| Campo | Dónde se incrementa | Quién lo hace |
|-------|-------------------|---------------|
| `completed_stays` | `increment_completed_stays` (core, CPI-gated) | Escrow vía `release_funds` |
| `hosted_stays` | `increment_hosted_stays` (core, CPI-gated) | Escrow vía `release_funds` |

Además, las cancelaciones incrementan contadores de reputación:

| Campo | Dónde se incrementa | Quién lo hace |
|-------|-------------------|---------------|
| `client_cancellations` | `increment_client_cancellations` (core) | Escrow vía `guest_cancel_booking` |
| `host_cancellations` | `increment_host_cancellations` (core) | Escrow vía `host_cancel_booking` |

## Configuración inicial

`GlobalConfig.free_ops` se define en `initialize_config`. Un valor de `0` desactiva
el free tier (todos los usuarios necesitan depósito desde la primera operación).

```rust
// stayke-config: initialize_config
pub fn initialize_config(
    ctx: Context<InitializeConfig>,
    minimum_deposit: u64,
    fee_bps: u64,
    free_ops: u8,  // ← umbral de operaciones gratuitas
) -> Result<()>
```

## Checklist de seguridad

- [x] `increment_completed_stays` y `increment_hosted_stays` solo pueden ser llamados por Escrow (CPI authority gated)
- [x] `completed_stays` y `hosted_stays` se incrementan con `saturating_add` (no overflow)
- [x] La comparación `free_ops as u32` es segura porque `free_ops` es `u8` (0–255)
- [x] El constraint se evalúa en deserialización (Anchor), sin runtime bypass posible
- [x] `clear_active_booking` acepta CPI de Escrow o Disputes (allowlist dual)
- [x] Cancelaciones incrementan contadores de reputación vía CPI (no contadores de free tier)
