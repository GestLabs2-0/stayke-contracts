# 📋 Stayke Contracts — TODO General y Especificaciones Pendientes

> Estado actual del código: todos los TODOs encontrados + especificaciones para nuevas features solicitadas.
> Basado en el análisis de los 5 programas Anchor: `stayke-config`, `stayke-core`, `stayke-escrow`, `stayke-disputes`, `stayke-treasury`.

---

## 🔍 Tabla de Contenidos

1. [TODOs Existentes en el Código](#1-todos-existentes-en-el-código)
2. [Nuevas Features Solicitadas](#2-nuevas-features-solicitadas)
   - [2.1 Eliminar identidad de UserProfile (Didit inválida)](#21-eliminar-identidad-de-userprofile-cuando-didit-informe-validación-falsa)
   - [2.2 Re-agregar identidad a UserProfile](#22-re-agregar-identidad-a-userprofile)
   - [2.3 Link a transacción Arweave/IPFS en Listing](#23-agregar-propiedad-que-linkee-a-transacción-en-arweave-o-ipfs-en-listing)
   - [2.4 BookingDays por año (reducir costos)](#24-actualizar-bookingdays-para-que-trabaje-por-año)
   - [2.5 Resolución de disputas entre usuarios + escalado a admin](#25-instrucciones-de-resolución-de-disputas-con-escalado-a-admin)
   - [2.6 Depósito obligatorio post 2da/3ra reserva u hosteo](#26-sistema-de-depósitos-obligatorio-tras-n-reservas-o-hostings)

---

## 1. TODOs Existentes en el Código

### stayke-config

| # | Archivo | Línea | TODO | Prioridad |
|---|---------|-------|------|-----------|
| 1 | `programs/stayke-config/src/state.rs` | 3 | **add stayke contracts to Global Config** — Agregar los Pubkeys oficiales de los otros 4 programas en `GlobalConfig` para que sea la única fuente de verdad (single source of truth) para CPIs. Actualmente no se valida que los programas llamantes sean los autorizados. | 🔴 CRITICAL |
| 2 | `programs/stayke-config/src/lib.rs` | 27 | **create instruction to withdraw fees from vault** — No existe una instrucción para que el admin pueda retirar las comisiones acumuladas en el `platform_vault`. El sistema recauda fees pero no tiene cómo extraerlos. | 🟡 HIGH |

### stayke-core

| # | Archivo | Línea | TODO | Prioridad |
|---|---------|-------|------|-----------|
| 3 | `programs/stayke-core/src/instructions/cpi/user_profile_mutators.rs` | 7 | **enforce security. We don't allow modifications from other contracts unless we secure them beforehand** — Las funciones mutadoras (`update_deposit`, `clear_active_booking`) no validan que la `authority` firmante sea un PDA autorizado. Cualquier contrato (o incluso un user directo) podría llamarlas si tiene la seed correcta. | 🔴 CRITICAL |
| 4 | `programs/stayke-core/src/instructions/cpi/clear_listing_booking.rs` | 8 | **enforce security...** — Misma vulnerabilidad que #3. La instrucción `clear_listing_booking` no verifica que quien la llama sea un programa autorizado (ej. Escrow o Disputes). | 🔴 CRITICAL |
| 5 | `programs/stayke-core/src/instructions/cpi/clear_listing_booking.rs` | 12 | **if the user_profile is the seed for the listing, then we can be sure...** — Reflexión sobre seguridad de seeds. La propiedad `is_occupied` de un Listing debería ser modificable solo por los contratos (Escrow/Disputes), no por el usuario directo. | 🟡 HIGH |

### stayke-escrow

| # | Archivo | Línea | TODO | Prioridad |
|---|---------|-------|------|-----------|
| 6 | `programs/stayke-escrow/src/state/bookings.rs` | 4 | **refactor bookings days to store all year instead of multiple accounts** — Actualmente se crea una cuenta `BookingDays` por mes. Esto es costoso en renta y complejidad. La idea es usar un bitmap de 365 días (o similar) en una sola cuenta por año. | 🟡 HIGH |
| 7 | `programs/stayke-escrow/src/instructions/create_booking.rs` | 79 | **check if it is required to use init_if_needed in the other bookingDays acc** — Revisar si `init_if_needed` es correcto para BookingDays cuando la reserva cruza de mes. | 🟢 MEDIUM |
| 8 | `programs/stayke-escrow/src/instructions/host_reject_booking.rs` | 13 | **change reputation if host rejects inside a 48 hours frame** — No se actualiza la reputación del host cuando rechaza una reserva dentro de las 48h previas al check-in. | 🟢 MEDIUM |
| 9 | `programs/stayke-escrow/src/instructions/host_reject_booking.rs` | 36 | **check if lamports for closing booking goes to payer** — Verificar que al cerrar la cuenta Booking los lamports de renta vayan al payer correcto. | 🟢 MEDIUM |
| 10 | `programs/stayke-escrow/src/instructions/client_reject_reserve.rs` | 13 | **change reputation if client rejects inside a 48 hours frame** — Mismo caso que #8 pero para el cliente. | 🟢 MEDIUM |
| 11 | `programs/stayke-escrow/src/instructions/complete_stay.rs` | 112 | **should I implement some kind of conditional if the host is banned. What happens to the money if the host is banned after the stay is completed but before the booking is closed?** — Caso borde: si el host es baneado entre que el guest completa el review y se ejecuta `complete_stay`, los fondos se transfieren igual al host. Habría que redirigirlos o congelarlos. | 🟡 HIGH |

### stayke-disputes

| # | Archivo | Línea | TODO | Prioridad |
|---|---------|-------|------|-----------|
| 12 | `programs/stayke-disputes/src/instructions/resolve_dispute.rs` | 51 | **add validations for token accounts. Platform and usdc_mint need to be equal to the other config files** — Las cuentas de token en `resolve_dispute` no se validan contra `GlobalConfig`. Un admin malicioso o un error podría desviar fondos. | 🔴 CRITICAL |
| 13 | `programs/stayke-disputes/src/instructions/close_dispute.rs` | 67 | **instead of the admin users, we must only use the account PDA as the signer, but for simplicity we can just use the admin signer for now.** — El close_dispute usa un admin signer (centralizado). Debería usar el PDA del programa como authority para firmar CPIs autónomamente. | 🟡 HIGH |

### stayke-treasury

| # | Archivo | Línea | TODO | Prioridad |
|---|---------|-------|------|-----------|
| 14 | `programs/stayke-treasury/src/instructions/lending.rs` | 8 | **In the future this instruction will allow users to lend their USDC** | 🟢 MEDIUM |
| 15 | `programs/stayke-treasury/src/instructions/lending.rs` | 15 | **Add lending protocol accounts** | 🟢 MEDIUM |
| 16 | `programs/stayke-treasury/src/instructions/lending.rs` | 25 | **In the future this instruction will allow users to recall their USDC** | 🟢 MEDIUM |
| 17 | `programs/stayke-treasury/src/instructions/lending.rs` | 31 | **Add lending protocol accounts** | 🟢 MEDIUM |
| 18 | `programs/stayke-treasury/src/instructions/lending.rs` | 44 | **Liquid staking placeholder** | 🟢 MEDIUM |
| 19 | `programs/stayke-treasury/src/instructions/lending.rs` | 50 | **Add staking protocol accounts** | 🟢 MEDIUM |

> **Nota:** Los items 14-19 son placeholders para DeFi (lending/staking). El código está comentado en `lib.rs`.

### bugs / code_quality (no TODO explícito pero detectados)

| # | Descripción | Archivo | Prioridad |
|---|-------------|---------|-----------|
| 20 | `close_booking.rs` línea 40: el constraint usa `host_reputation.bump` pero la cuenta es `host_profile` (UserProfile). El segundo constraint línea 43-48 pide `host_profile` como UserProfile con seeds de ReputationProfile. **Las seeds están invertidas**: `host_reputation` se deriva con seed de UserProfile y `host_profile` con seed de ReputationProfile. | `stayke-escrow/src/instructions/close_booking.rs` | 🔴 CRITICAL |
| 21 | El `active_booking` en `UserProfile` se setea en `create_booking` pero nunca se actualiza — no hay código que asigne el valor. Solo se limpia con `clear_active_booking`. El campo existe pero nunca se escribe durante la creación de la reserva. | `stayke-core/src/state/users.rs`, `stayke-escrow/src/instructions/create_booking.rs` | 🟡 HIGH |
| 22 | `PenalizeUser` en disputes no valida que el `treasury_config` y `global_config` pasados sean correctos (son `UncheckedAccount`). Un admin podría pasar cuentas falsas. | `programs/stayke-disputes/src/instructions/penalize_user.rs` | 🔴 CRITICAL |
| 23 | No existe instrucción `unlink_identity` — una vez linkeada una identidad a un UserProfile, no hay forma de desvincularla. Si Didit reporta que la validación fue falsa, el contrato no puede reaccionar. | `stayke-core/src/lib.rs` | 🟡 HIGH |
| 24 | `months_days()` en utils.rs no considera años bisiestos (Febrero siempre 28). | `stayke-escrow/src/utils.rs` | 🟢 MEDIUM |

---

## 2. Nuevas Features Solicitadas

### 2.1 Eliminar identidad de UserProfile (cuando Didit informe validación falsa)

**Contexto actual:**
- `link_identity` asigna `user_profile.identity = Some(identity.key())` y marca `identity.linked = true`.
- No existe una instrucción inversa. Si Didit (servicio KYC) detecta que una validación fue falsa, el contrato no puede desvincular la identidad.

**Especificación:**

```rust
// --- Nueva instrucción en stayke-core ---

// Nombre: unlink_identity
// Actor: authority del config (admin/backend)
// Pre-condiciones:
//   1. user_profile.identity == Some(identity_pubkey)
//   2. identity.linked == true
// Post-condiciones:
//   1. user_profile.identity = None
//   2. identity.linked = false
//   3. (Opcional) identity.verified_at se resetea a 0

// Consideraciones de seguridad:
// - Solo el authority del ConfigAcc puede ejecutarla (no el usuario).
// - Si el user_profile tiene un active_booking, ¿se permite desvincular?
//   Propuesta: NO permitir si active_booking es Some (el usuario está en medio de una reserva).
// - Si el user_profile está banned, igual se debe poder desvincular (para limpieza).
```

**Archivos a modificar:**
- `programs/stayke-core/src/instructions/` → nuevo archivo `unlink_identity.rs`
- `programs/stayke-core/src/instructions.rs` → agregar módulo
- `programs/stayke-core/src/lib.rs` → registrar instrucción pública
- `programs/stayke-core/src/error.rs` → agregar errores si es necesario

**Modelo de datos** (no requiere cambios — Identity ya tiene `linked: bool`, UserProfile ya tiene `identity: Option<Pubkey>`).

---

### 2.2 Re-agregar identidad a UserProfile

**Contexto actual:**
- `link_identity` tiene el constraint `!user_profile.identity.is_some()` — no permite relinkear.
- Una vez que se desvincula con `unlink_identity` (item 2.1), no se puede volver a linkear la misma identidad (porque `identity.linked` sigue en `true`).

**Especificación:**

```rust
// --- Modificación a link_identity existente ---
// O nueva instrucción: relink_identity

// Opción A: Modificar constraint de link_identity
//   Cambiar: !user_profile.identity.is_some()
//   Por: !user_profile.banned (solo no permitir si está baneado)
//   Y además permitir relink si identity.linked == false después de un unlink.

// Opción B: Nueva instrucción relink_identity
//   Pre-condiciones:
//     1. user_profile.identity == None (fue desvinculado)
//     2. identity.linked == false
//     3. !user_profile.banned
//   Post-condiciones:
//     1. user_profile.identity = Some(identity.key())
//     2. identity.linked = true
//     3. identity.verified_at = Clock::now() (nueva verificación)

// Recomendación: Opción B es más explícita y segura.
// Separar "link inicial" de "relink después de fraude" permite
// diferentes reglas de negocio.
```

**Archivos a modificar:**
- `programs/stayke-core/src/instructions/` → nuevo archivo `relink_identity.rs`
- `programs/stayke-core/src/instructions.rs` → agregar módulo
- `programs/stayke-core/src/lib.rs` → registrar instrucción
- Opcional: modificar `link_identity.rs` constraints

---

### 2.3 Agregar propiedad que linkee a transacción en Arweave o IPFS en Listing

**Contexto actual:**
- `Listing` solo tiene `price`, `state_hash`, `is_occupied`, `owner`, `listing_id`, `rating`, `total_reviews`.
- No hay forma de almacenar una referencia a los metadatos off-chain (imágenes, descripción, ubicación).
- Actualmente se usa `state_hash: [u8; 32]` para verificar integridad, pero no apunta a dónde están los datos.

**Especificación:**

```rust
// --- Modificar struct Listing ---

pub struct Listing {
    pub owner: Pubkey,
    pub listing_id: u16,
    pub total_reviews: u64,
    pub rating: u64,
    pub price: u64,
    pub is_occupied: bool,
    pub state_hash: [u8; 32],

    // NUEVO: URI pointing to metadata on Arweave/IPFS/backend
    // Ej: "ar://<tx-id>" o "ipfs://<cid>" o "https://api.stayke.com/metadata/<id>"
    pub metadata_uri: String,  // o [u8; 64] si se prefiere fixed-size
    // O simplemente expandir state_hash para incluir el commitment del metadata + URI hash
}

// Consideraciones:
// - Anchor String requiere espacio dinámico → aumenta renta.
// - Alternativa: almacenar solo el hash del URI + mantener el URI real en un evento.
// - Alternativa 2: Usar un mapping account (MetadataAccount) separado del Listing
//   para no incrementar el espacio del Listing principal.
// Recomendación inicial: Usar un campo String con max_len (ej. 128 bytes).
```

**Seed de la cuenta Listing:** No cambia. El `metadata_uri` es un campo adicional, no afecta la derivación.

**Archivos a modificar:**
- `programs/stayke-core/src/state/listings.rs` → agregar campo
- `programs/stayke-core/src/instructions/initialize_listing.rs` → recibir `metadata_uri` como parámetro, guardarlo en el init
- `programs/stayke-core/src/instructions/listing_mutator.rs` → agregar `handler_update_listing_metadata` o modificar `handler_update_listing_state`
- `packages/` → regenerar typescript bindings

---

### 2.4 Actualizar BookingDays para que trabaje por año

**Contexto actual:**
- `BookingDays` almacena un bitmap de 32 bits para los días ocupados de un mes específico:
  ```rust
  pub struct BookingDays {
      pub property: Pubkey,
      pub occupied_days: u32,  // bitmap, día 1 = bit 0
      pub month: u32,
      pub year: u32,
      pub initialized: bool,
      pub bump: u8,
  }
  ```
- Se crea una cuenta **por mes** que la reserva abarque. Si una reserva va del 25/01 al 05/02, se necesitan 2 cuentas BookingDays.
- El TODO original dice: _"refactor bookings days to store all year instead of multiple accounts"_.

**Especificación:**

```rust
// --- Nueva estructura BookingDays (anual) ---

pub struct BookingDays {
    pub property: Pubkey,
    pub occupied_days: [u32; 12],  // 12 bitmaps, uno por mes
    pub year: u32,                  // El año que cubre esta cuenta
    pub bump: u8,
}

// O usando Bigger bitmap (365 bits ~ 6 x u64):
pub struct BookingDays {
    pub property: Pubkey,
    pub occupied_days: [u64; 6],  // 384 bits para cubrir 366 días (bisiesto)
    pub year: u32,
    pub bump: u8,
}

// Seed: BOOKING_DAYS_SEED + property + year
// seeds = [b"booking_days", property.key().as_ref(), year.to_le_bytes().as_ref()]

// Ventajas:
// 1. Una sola cuenta por año por propiedad (vs una por mes).
// 2. Menos cuentas PDA → menos renta, menos transacciones.
// 3. Reservas que cruzan meses ya no necesitan remaining_accounts complejos.
// 4. Lógica más simple en reserve_days() y release_days().

// Desventajas:
// - La cuenta es más grande (~56 bytes + 48/24 bytes de bitmaps = ~104/80 bytes).
//   VS antes: ~50 bytes por cuenta × 12 meses = ~600 bytes si se crearan todas.
//   En la práctica, solo se creaban las necesarias, pero igual el ahorro de renta
//   es significativo porque NO se pagan 12 cuentas, solo 1 por año.

// Seed changes:
// ANTES: seeds = [BOOKING_DAYS_SEED, property, year_month]
// AHORA: seeds = [BOOKING_DAYS_SEED, property, year]
```

**Archivos a modificar:**
- `programs/stayke-escrow/src/state/bookings.rs` → reestructurar `BookingDays`
- `programs/stayke-escrow/src/utils.rs` → refactorizar `reserve_days()`, `release_days()`, `bitmap_days()`, `months_days()`
- `programs/stayke-escrow/src/instructions/create_booking.rs` → ajustar seeds de `booking_days`
- `programs/stayke-escrow/src/instructions/host_reject_booking.rs` → ajustar seeds
- `programs/stayke-escrow/src/instructions/client_reject_reserve.rs` → ajustar seeds

---

### 2.5 Instrucciones de resolución de disputas con escalado a admin

**Contexto actual:**
- `open_dispute`: guest o host abre una disputa. El booking pasa a estado `Disputed`.
- `resolve_dispute`: solo un admin (de `DisputeConfig.admins`) puede resolverla.
- `close_dispute`: solo un admin puede cerrarla y hacer cleanup.
- **No existe** un flujo donde los usuarios puedan resolver la disputa entre sí antes de escalar a admin.

**Especificación:**

```rust
// --- Nueva instrucción: propose_resolution (usuario) ---
// Cualquiera de las partes (initiator o guilty) propone una resolución.
// La otra parte debe accept_resolution para que se ejecute.

// --- Nueva instrucción: accept_resolution (usuario) ---
// La contraparte acepta la resolución propuesta.
// La disputa se resuelve sin intervención de admin.

// --- Nueva instrucción: escalate_dispute (usuario) ---
// Si pasa X tiempo sin acuerdo, cualquiera de las partes puede escalar
// la disputa a un admin para que la resuelva.

// --- Modificar DisputeStatus ---
pub enum DisputeStatus {
    Open,                    // Recién abierta
    ResolutionProposed,      // Un usuario propuso una resolución
    AwaitingEscalation,      // Se solicitó escalado a admin
    Resolved,                // Resuelta por acuerdo entre usuarios
    Rejected,                // Rechazada
    AdminResolved,           // Resuelta por admin (la actual Resolved)
}

// --- Nueva estructura de Dispute ---
// Se agregan campos para manejar el timeline:
pub struct Dispute {
    pub booking: Pubkey,
    pub property: Pubkey,
    pub initiator: Pubkey,
    pub guilty: Pubkey,
    pub reason: DisputeReason,
    pub status: DisputeStatus,
    pub created_at: i64,
    pub resolved_at: Option<i64>,

    // NUEVOS
    pub proposed_resolution: Option<Resolution>,  // propuesta actual
    pub escalated_at: Option<i64>,                // cuándo se escaló
    pub timeout_days: u32,                        // días antes de escalar automáticamente
}

pub enum Resolution {
    FullRefund,
    PartialRefund(u16),      // bps para el guest
    FullPaymentToHost,
    Custom(u16, u16),        // guest_share_bps, host_share_bps
}
```

**Lógica de escalado:**

```
Time = 0:  open_dispute          → estado: Open
Time = 24h: Sin acuerdo          → cualquiera puede escalate_dispute → estado: AwaitingEscalation
Time = 48h: Sin acción admin     → (opcional) el sistema automáticamente libera la disputa
            (off-chain oracle o cualquiera puede llamar force_resolve)

O bien:

Time = 0:   open_dispute
Time = 24h: propose_resolution (initiator)
Time = 48h: Si la otra parte no accept, se puede escalate_dispute
Time = 72h: Admin toma el caso con resolve_dispute (existente)
```

**Archivos nuevos/modificar:**
- `programs/stayke-disputes/src/instructions/` → nuevos archivos:
  - `propose_resolution.rs`
  - `accept_resolution.rs`
  - `escalate_dispute.rs`
- `programs/stayke-disputes/src/state/disputes.rs` → modificar `DisputeStatus` y `Dispute`
- `programs/stayke-disputes/src/instructions.rs` → agregar módulos
- `programs/stayke-disputes/src/lib.rs` → registrar instrucciones
- `programs/stayke-disputes/src/error.rs` → nuevos errores

---

### 2.6 Sistema de depósitos obligatorio tras N reservas o hostings

**Contexto actual:**
- `GlobalConfig.minimum_deposit` es fijo. Todos los usuarios deben tener `deposited >= minimum_deposit` para crear reservas.
- No hay diferenciación entre usuarios nuevos y veteranos.
- `create_booking` valida `client_profile.deposited >= global_config.minimum_deposit` y lo mismo para `host_profile`.

**Especificación:**

```rust
// --- Modificar GlobalConfig ---
pub struct GlobalConfig {
    pub authority: Pubkey,
    pub minimum_deposit: u64,         // depósito base (nuevos usuarios)
    pub fee_bps: u64,
    pub usdc_mint: Pubkey,
    pub is_initialized: bool,
    pub platform_vault: Pubkey,
    pub platform_vault_bump: u8,
    pub bump: u8,

    // NUEVOS
    pub free_tier_bookings: u32,      // cantidad de bookings "gratis" sin depósito (ej: 2)
    pub free_tier_hostings: u32,      // cantidad de hostings "gratis" sin depósito (ej: 3)
}

// --- Modificar UserProfile ---
pub struct UserProfile {
    pub authority: Pubkey,
    pub identity: Option<Pubkey>,
    pub active_booking: Option<Pubkey>,
    pub deposited: u64,
    pub lending: u64,
    pub staked: u64,
    pub banned: bool,
    pub listings: u16,
    pub bump: u8,

    // NUEVOS (o usar ReputationProfile)
    pub total_bookings: u32,      // contador de bookings hechos como guest
    pub total_hostings: u32,      // contador de hostings completados
}

// --- Lógica ---

// En create_booking:
//   let min_required = if client_profile.total_bookings < global_config.free_tier_bookings {
//       0  // aún en "free tier", no requiere depósito
//   } else {
//       global_config.minimum_deposit
//   };
//   require!(client_profile.deposited >= min_required, ...);
//
//   Después de crear la reserva:
//   client_profile.total_bookings += 1;

// En complete_stay (para host):
//   host_profile.total_hostings += 1;

// En withdraw_guarantee:
//   Se permite retirar solo si deposited - amount >= min_required (según el tier del usuario)
//   O permitir retirar siempre que no haya active_booking (como ahora) y el usuario
//   entienda que no podrá hacer nuevas reservas si no tiene el mínimo.
```

**Alternativa más simple (recomendada):**

```rust
// Flags discretos en UserProfile en vez de contadores:
pub struct UserProfile {
    // ... existentes ...
    pub deposit_required: bool,  // true si ya usó sus "free tier" gratis
}

// Lógica:
// 1. deposit_required empieza en false.
// 2. En create_booking: si deposit_required == false, se permite sin depósito.
//    Luego de N reservas, se setea deposit_required = true.
// 3. Con deposit_required == true, se exige deposited >= minimum_deposit.
```

**Archivos a modificar:**
- `programs/stayke-config/src/state.rs` → agregar `free_tier_bookings`, `free_tier_hostings`
- `programs/stayke-core/src/state/users.rs` → agregar contadores o flag
- `programs/stayke-escrow/src/instructions/create_booking.rs` → lógica de tier
- `programs/stayke-escrow/src/instructions/host_accept_booking.rs` → validación de tier para host
- `programs/stayke-escrow/src/instructions/client_accept_reserve.rs` → validación

---

## Apéndice: Resumen de Prioridades

| Prioridad | Cantidad | Items |
|-----------|----------|-------|
| 🔴 CRITICAL | 5 | #1, #3, #4, #12, #20, #22 |
| 🟡 HIGH | 5 | #2, #5, #11, #13, #21 |
| 🟢 MEDIUM | 9 | #7, #8, #9, #10, #14-19, #24 |

## Apéndice: Mapa de Programas y sus Interacciones

```
stayke-config (GlobalConfig) ←── Fuente de verdad central
    ├── stayke-core (UserProfile, Listing, Identity, ReputationProfile)
    │     └── CPI: Escrow, Disputes, Treasury lo llaman para mutar perfiles
    ├── stayke-escrow (Booking, BookingDays, EscrowConfig)
    │     └── CPI: Disputes lo llama para congelar/repartir fondos
    ├── stayke-disputes (Dispute, DisputeConfig)
    │     └── CPI: Llama a Core (infractions), Escrow (status/transfer), Treasury (penalize)
    └── stayke-treasury (TreasuryConfig, Treasury Vault)
          └── CPI: Llama a Core (update_deposit), recibe CPI de Disputes
```
