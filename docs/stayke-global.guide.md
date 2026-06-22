# 📖 Referencia Técnica Global — stayke-contracts

> Documentación técnica detallada: program IDs, seeds, cuentas, campos y relaciones CPI.
> Complementa las guías conceptuales de cada programa.

---

## 🆔 Program IDs

| Programa | ID |
|---|---|
| `stayke-config` | `2GM2yLmDtz2Hyb8T5VBftERmiyJ5whKUmv6V4hBjNXMW` |
| `stayke-core` | `8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP` |
| `stayke-escrow` | `FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ` |
| `stayke-disputes` | `7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB` |
| `stayke-treasury` | `59buEPHFBK4h8LyLE2KtnV1kpaQTyjb82NWt5F9jSuHu` |
| `stayke_contracts` (Anchor.toml localnet) | `4fyRhe1g8fJjHRxLAS9vT1RLjS44W3FutzF9USXAdNtB` |

---

## 📦 Accounts, Seeds y Campos por Programa

### 1. stayke-config

**`GlobalConfig`** — seed: `"global_config"`

| Campo | Tipo | Descripción |
|---|---|---|
| `authority` | `Pubkey` | Admin del protocolo |
| `minimum_deposit` | `u64` | Depósito mínimo para operar |
| `fee_bps` | `u64` | Comisión plataforma en basis points |
| `usdc_mint` | `Pubkey` | Token mint (USDC) |
| `is_initialized` | `bool` | Guard flag |
| `platform_vault` | `Pubkey` | Token account vault de comisiones |
| `platform_vault_bump` | `u8` | Bump del vault PDA |
| `bump` | `u8` | Bump de GlobalConfig |

Seeds adicionales: `"platform_vault"`, `"platform_vault_token"`

---

### 2. stayke-core

**`ConfigAcc`** — seed: `"config"`

| Campo | Tipo |
|---|---|
| `authority` | `Pubkey` |
| `bump` | `u8` |

**`UserProfile`** — seed: `"user_profile"` / `[authority]`

| Campo | Tipo | Descripción |
|---|---|---|
| `owner` | `Pubkey` | Wallet del usuario |
| `identity` | `Pubkey` | Link a Identity account |
| `active_booking` | `Option<Pubkey>` | Booking activo como guest |
| `active_stay` | `Option<Pubkey>` | Stay activo como host |
| `deposited` | `u64` | Total depositado en treasury |
| `deposit_timestamp` | `i64` | UNIX timestamp |
| `lending` | `u64` | Monto en lending (futuro) |
| `staked` | `u64` | Monto staked (futuro) |
| `is_verified` | `bool` | KYC aprobado |
| `banned` | `bool` | Cache del flag en Identity |
| `listings` | `u16` | Contador para listing_id |
| `bump` | `u8` | |

**`ReputationProfile`** — seed: `"reputation_profile"` / `[authority]`

| Campo | Tipo |
|---|---|
| `owner` | `Pubkey` |
| `host_reviews` / `total_score_host` | `u32` / `u64` |
| `client_reviews` / `total_score_client` | `u32` / `u64` |
| `hosted_stays` / `completed_stays` | `u32` / `u32` |
| `host_cancellations` / `client_cancellations` | `u32` / `u32` |
| `host_cancellations_within_48h` / `client_cancellations_within_48h` | `u32` / `u32` |
| `low_infractions` / `medium_infractions` / `high_infractions` | `u8` / `u8` / `u8` |
| `last_updated` | `i64` |
| `bump` | `u8` |

**`Identity`** — seed: `"identity"` / `[id_hash]`

| Campo | Tipo |
|---|---|
| `owner` | `Pubkey` |
| `country_code` | `[u8; 2]` |
| `id` | `[u8; 32]` |
| `verified_at` | `i64` |
| `verifier` | `Option<Pubkey>` |
| `doc_type` | `DocType` (Passport | DriversLicense | IdCard) |
| `is_frozen` | `bool` |
| `is_banned` | `bool` |
| `banned_at` | `i64` |
| `bump` | `u8` |

**`Listing`** — seed: `"listing"` / `[owner_profile]` / `[listing_id: u16 LE]`

| Campo | Tipo |
|---|---|
| `owner` | `Pubkey` |
| `listing_id` | `u16` |
| `total_reviews` | `u64` |
| `rating` | `u64` |
| `price` | `u64` |
| `is_occupied` | `Option<Pubkey>` (guest) |
| `state_hash` | `[u8; 32]` |
| `bump` | `u8` |

---

### 3. stayke-escrow

**`EscrowConfig`** — seed: `"escrow_config"`

| Campo | Tipo |
|---|---|
| `authority` | `Pubkey` |
| `global_config` | `Pubkey` |
| `is_initialized` | `bool` |
| `bump` | `u8` |

**`Booking`** — seed: `"booking"` / `[property]` / `[guest_profile]` / `[check_in: i64 LE]`

| Campo | Tipo | Descripción |
|---|---|---|
| `guest` | `Pubkey` | Guest UserProfile key |
| `host` | `Pubkey` | Host UserProfile key |
| `property` | `Pubkey` | Listing key |
| `deposit` | `u64` | (sin uso aún) |
| `check_in` / `check_out` | `i64` | UNIX timestamps |
| `days` | `u64` | Duración en días |
| `check_in_date` / `check_out_date` | `DateComponents` | day/month/year |
| `total_price` | `u64` | price * days |
| `review` | `u8` | 0 = no review, 1–5 |
| `status` | `BookingStatus` | Ver abajo |
| `escrow_bump` | `u8` | Bump del escrow token account |
| `bump` | `u8` | Bump del booking |

**`BookingStatus`** — enum:
`Pending` → `HostAccepted` → `Active` → `ReviewCompleted` → `Completed`
│         → `Cancelled`
│         → `Disputed` → `DisputeResolved` | `DisputeRejected`

**`BookingDays`** — seed: `"booking_days"` / `[property]` / `[year_month: u32 LE]`

| Campo | Tipo | Descripción |
|---|---|---|
| `property` | `Pubkey` | Listing key |
| `occupied_days` | `u32` | Bitmask (bit n = día n+1) |
| `month` | `u32` | 1–12 |
| `year` | `u32` | |
| `initialized` | `bool` | Evita double-counting |
| `bump` | `u8` | |

---

### 4. stayke-disputes

**`DisputeConfig`** — seed: `"dispute_config"`

| Campo | Tipo |
|---|---|
| `admins` | `Vec<Pubkey>` (max 5) |
| `retribution_bps_low/medium/high` | `u16` |
| `is_initialized` | `bool` |
| `bump` | `u8` |

**`Dispute`** — seed: `"dispute"` / `[booking]`

| Campo | Tipo |
|---|---|
| `booking` | `Pubkey` |
| `property` | `Pubkey` |
| `initiator` | `Pubkey` |
| `guilty` | `Pubkey` |
| `reason` | `DisputeReason` |
| `status` | `DisputeStatus` (Open | Resolved | Rejected) |
| `created_at` | `i64` |
| `resolved_at` | `Option<i64>` |
| `bump` | `u8` |

**`DisputeReason`** — enum: `PropertyNotAsDescribed`, `HostUnreachable`, `GuestDamagedProperty`, `GuestBrokeRules`, `Other`

---

### 5. stayke-treasury

**`TreasuryConfig`** — seed: `"treasury_config"`

| Campo | Tipo |
|---|---|
| `authority` | `Pubkey` |
| `treasury_vault` | `Pubkey` |
| `treasury_bump` | `u8` |
| `global_config` | `Pubkey` |
| `is_initialized` | `bool` |
| `bump` | `u8` |

Seeds adicionales: `"treasury"` (PDA que firma transfers), `"treasury_vault"`

---

## 🔗 Mapa de Relaciones CPI

### Treasury → Core
```
deposit_guarantee ──CPI──> stayke_core::cpi::update_deposit(amount, is_deposit=true)
withdraw_guarantee ──CPI──> stayke_core::cpi::update_deposit(amount, is_deposit=false)
```

### Disputes → Escrow
```
open_dispute ──CPI──> stayke_escrow::cpi::cpi_update_booking_status(Disputed)
resolve_dispute ──CPI──> stayke_escrow::cpi::cpi_resolve_dispute_transfer(host_share_bps, rejected)
```

### Disputes → Core
```
penalize_user ──CPI──> stayke_core::cpi::add_infraction(severity)
close_dispute ──CPI──> stayke_core::cpi::clear_active_booking() (guest + host)
close_dispute ──CPI──> stayke_core::cpi::clear_listing_booking()
```

### Disputes → Treasury (NO implementado — TODO)
```
resolve_dispute ──??──> stayke_treasury::cpi::cpi_penalize_transfer(amount)
```

### Lectura de Config (todas → stayke-config)
Todos los programas referencian `GlobalConfig` de stayke-config para validar:
- `usdc_mint` (token correcto)
- `platform_vault` (vault de comisiones)
- `fee_bps` (comisión)
- `minimum_deposit`
- `global_config.key()` en constraint (con seeds::program)

### Escrow → Core (lectura de estado)
```
create_booking ──lee──> UserProfile (guest + host): verified, banned, deposited
host_accept_booking ──lee──> UserProfile (host)
client_accept_reserve ──lee──> UserProfile (guest) + Listing
complete_stay ──lee──> UserProfile (guest + host)
review_completed ──escribe──> ReputationProfile (host_reviews, total_score_host, hosted_stays)
```

---

## 🗺️ Booking Lifecycle Completo

```
create_booking (guest)
    │ Pending
    ├── host_accept_booking ──> HostAccepted
    │       ├── client_accept_reserve ──> Active (USDC a escrow)
    │       │       ├── review_completed ──> ReviewCompleted
    │       │       │       └── complete_stay ──> Completed (USDC distribuido)
    │       │       └── (dispute) open_dispute ──> Disputed
    │       │               ├── resolve_dispute ──> DisputeResolved | DisputeRejected
    │       │               └── close_dispute (cleanup)
    │       └── client_reject_reserve ──> Cancelled
    └── host_reject_booking ──> Cancelled
```

---

## ⚙️ Platform Config

- **Cluster**: localnet
- **Wallet**: `./test-keypair.json`
- **Package manager**: yarn (Anchor.toml) / pnpm (workspace)
- **Language**: Rust + TypeScript SDKs
- **SDK packages**: `@GestLabs2-0/stayke-*` — publicados a GitHub Packages
- **SDK dependency**: `@solana/kit` ^6.4.0

---

## 🚧 TODOs Técnicos Pendientes

1. **Registrar program IDs en GlobalConfig** — para validar seeds::program en CPIs sin hardcodear
2. **Instrucción de withdraw fees** — retirar comisiones acumuladas del platform_vault
3. **cpi_penalize_transfer desde disputes** — el endpoint existe en treasury pero disputes aún no lo llama
4. **Validación de token accounts en resolve_dispute** — faltan constraints sobre mint y vault
5. **Caso borde: host baneado con fondos en escrow** — qué pasa si ban mean entre complete_stay y review
6. **Lending/Staking** — placeholders en treasury sin implementar
7. **PDA signing en disputes** — usar el PDA en vez de admin signer para firmar CPIs (actualmente usa admin wallet)
