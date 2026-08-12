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
| `stayke-escrow` | Aplica la validación condicional en `create_booking`, `host_accept_booking` y `client_accept_reserve`; incrementa `completed_stays` vía CPI en `complete_stay` |

## Flujo de coordinación

```
┌──────────────────────────────────────────────────────────────┐
│                     STAYKE-CONFIG                            │
│  GlobalConfig { free_ops, minimum_deposit }                  │
│  (PDA ["global_config"])                                    │
└────────────┬──────────────────────────────┬─────────────────┘
             │ read                         │ read
             ▼                              ▼
┌────────────────────────┐    ┌────────────────────────────────┐
│     STAYKE-CORE         │    │        STAYKE-ESCROW            │
│ UserProfile {           │    │                                 │
│   completed_stays,      │◄───│ complete_stay ──CPI──►          │
│   hosted_stays,         │    │   increment_completed_stays     │
│   deposited             │    │                                 │
│ }                       │    │ create_booking /                │
│                         │    │ host_accept_booking /            │
│                         │    │ client_accept_reserve            │
│                         │    │   validación condicional:        │
│                         │    │   (completed+hosted) < free_ops  │
│                         │    │   || deposited >= minimum_deposit│
└────────────────────────┘    └────────────────────────────────┘
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

Misma lógica para el host en `create_booking`, `host_accept_booking` y `client_accept_reserve`.

## CPI: incremento de `completed_stays`

Cuando un stay se completa (`complete_stay` en escrow), el programa llama vía CPI a
`stayke-core::increment_completed_stays`:

1. **Escrow** distribuye los fondos (host + fee) y cierra la cuenta de escrow.
2. **CPI** con signer seeds (`["cpi_authority"]`) llama a core.
3. **Core** verifica que el caller es Escrow (`assert_cpi_authority` con `AllowedCaller::Escrow`).
4. **Core** incrementa `user_profile.completed_stays` con `saturating_add(1)`.

```rust
// complete_stay.rs — después de distribuir fondos
let signer_seeds: &[&[&[u8]]] = &[&[CPI_AUTHORITY_SEED.as_bytes(), &[bump]]];
stayke_core::cpi::increment_completed_stays(
    CpiContext::new_with_signer(
        ctx.accounts.stayke_core_program.key(),
        stayke_core::cpi::accounts::UpdateUserProfile { ... },
        signer_seeds,
    ),
)?;
```

## Contadores existentes

| Campo | Dónde se incrementa | Quién lo hace |
|-------|-------------------|---------------|
| `hosted_stays` | `update_host_review` (core, CPI-gated) | Escrow vía `close_booking` |
| `completed_stays` | `increment_completed_stays` (core, CPI-gated) | Escrow vía `complete_stay` |

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

- [x] `increment_completed_stays` solo puede ser llamado por Escrow (CPI authority gated)
- [x] `completed_stays` y `hosted_stays` se incrementan con `saturating_add` (no overflow)
- [x] La comparación `free_ops as u32` es segura porque `free_ops` es `u8` (0–255)
- [x] El constraint se evalúa en deserialización (Anchor), sin runtime bypass posible
