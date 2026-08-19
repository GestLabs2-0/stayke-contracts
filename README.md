# Stayke Contracts

On-chain protocol for short-stay rentals on Solana. Stayke manages listings, user
profiles, escrowed bookings, guarantee deposits and dispute resolution across a set
of cooperative Anchor programs that talk to each other via CPI (Cross-Program Invocation).

> **Status:** development. The protocol uses USDC for deposits and platform fees, and
> the escrow holds booking funds until a stay completes or a dispute resolves it.

## Architecture overview

Stayke is split into **five Solana programs** (crates under `programs/`), each with a
single responsibility, so changes and audits stay scoped:

| Program | ID | Responsibility |
| --- | --- | --- |
| [`stayke-config`](#stayke-config) | `9ESE5Ztpr8zWbLyXCyiB5QqcjxHghotT8zqJxD2S3zaT` | Shared protocol configuration |
| [`stayke-core`](#stayke-core) | `2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm` | Users, listings and reputation |
| [`stayke-escrow`](#stayke-escrow) | `68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a` | Booking lifecycle and fund escrow |
| [`stayke-disputes`](#stayke-disputes) | `8vgDvWkdqhpGBPAczpmZ3DJahVgNN36soRnyw6MbfMCJ` | Dispute resolution and penalties |
| [`stayke-treasury`](#stayke-treasury) | `HV16vUTaZ78bJP1CyH5KDWyx8NqS1MYSGdPkRsMcnSuY` | Guarantee deposits and platform vault |

## Requirements

| Tool | Version | Notes |
| --- | --- | --- |
| [Rust](https://rustup.rs/) | `1.89.0` | Pinned via `rust-toolchain.toml` (rustfmt + clippy) |
| [Solana CLI](https://docs.anza.xyz/cli/) | `3.0.2` | |
| [Anchor](https://www.anchor-lang.com/) | `1.0.2` | |
| [Node.js](https://nodejs.org/) | `24` | For the TypeScript client packages |
| [pnpm](https://pnpm.io/) | `10.33.0` | Package manager for `packages/` |

The repo already carries a `Cargo.lock`, so builds are reproducible without a network
dependency lookup.

## Installation

### 1. Rust toolchain

```bash
# Installs the exact toolchain (channel 1.89.0) declared in rust-toolchain.toml
rustup toolchain install 1.89.0
rustup default 1.89.0
```

The `rust-toolchain.toml` file makes `cargo`/`rustc` pick this toolchain automatically
inside the repo, so the `rustup default` step is only needed to align your global default.

### 2. Solana CLI

```bash
sh -c "$(curl -sSfL https://release.anza.xyz/v3.0.2/install)"
```

Verify with `solana --version`. Solana is required by `anchor` to build and deploy.

### 3. Anchor CLI

```bash
cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked
```

Verify with `anchor --version` (should report `1.0.2`).

> **Note:** a mismatched Anchor version will fail with a version error at build time.
> If you have a different Anchor installed, align it with the version used in CI (`1.0.2`).

### 4. Node / pnpm (only needed for the TypeScript packages)

```bash
# Node 24 — e.g. via nvm (the repo pins this in .nvmrc)
nvm install 24.0.1
nvm use

# pnpm (repo pins a specific version in package.json packageManager)
corepack enable
corepack prepare pnpm@10.33.0 --activate

pnpm install
```

## Build

Build all Anchor programs and generate the on-chain IDLs:

```bash
anchor build --ignore-keys
```

Outputs:
- `target/deploy/*.so` — compiled Solana programs
- `target/idl/*.json` — generated IDLs (used by the TypeScript clients)

> `--ignore-keys` keeps the locally-declared program IDs (`Anchor.toml` / `declare_id!`)
> instead of generating new ones, so builds are deterministic across machines.

To build a single program:

```bash
anchor build stayke_escrow --ignore-keys
```

## Test

The programs are covered with Rust integration tests driven by **LiteSVM** (an in-process
Solana validator), declared as dev-dependencies in each crate:

```bash
cargo test --all-targets
```

To run a single crate's tests:

```bash
cargo test -p stayke-escrow
```

The CI pipeline (`cargo test --all-targets`, `cargo clippy --all-targets --tests -- -Dwarnings`
and `cargo +nightly fmt --all -- --check`) mirrors these commands, so running them locally
before a PR is a good smoke check.

## Lint & format

```bash
cargo clippy --all-targets --tests -- -Dwarnings
cargo +nightly fmt --all -- --check   # uses the nightly toolchain, per CI
```

## TypeScript packages

The `packages/` folder holds generated TypeScript clients (`@GestLabs2-0/stayke-*`) built
with `tsup` and published to GitHub Packages. Rebuild and publish all of them with:

```bash
pnpm --filter './packages/*' build
pnpm publish -r
```

These clients are regenerated from the Anchor IDLs and mirror the on-chain programs.

## Program summaries

### `stayke-config`

Shared configuration **single source of truth** for the whole protocol: minimum deposit,
platform fee (in basis points), the USDC mint, and the platform fee vault. Centralizing
these values here avoids hardcoding them across contracts.

- `initialize_config(minimum_deposit, fee_bps)` — admin; creates the `GlobalConfig` account
  (`authority`, `minimum_deposit`, `fee_bps`, `usdc_mint`, `platform_vault`, bumps).

> **Design note:** the intent is for every CPI call between Stayke contracts to validate
> the signer against this global config, instead of hardcoding program addresses. Both
> `stayke-escrow` and `stayke-treasury` currently reference `global_config` for fee
> calculation, mint validation, and vault checks.

### `stayke-core`

The **identity and marketplace** program. Manages user profiles, property listings, and the
reputation/penalty system shared across the protocol.

- `initialize_config` — admin; creates the core `ConfigAcc`.
- `initialize_user_profile` — creates a user profile (host or guest).
- `initialize_listing(price, listing_id)` — creates a property listing.
- `update_deposit(amount, is_deposit)` — add or release a user's deposit.
- `clear_active_booking` — frees a user's active booking slot.
- `add_infraction(severity)` — records a reputation infraction (`PenaltySeverity`).
- `clear_listing_booking` — releases a listing after a booking ends.
- `init_identity` / `link_identity` — identity verification flow.

### `stayke-escrow`

The **core transactional** program. Owns the full booking lifecycle and escrows the funds
(USDC) between guest and host until the stay completes.

#### Booking status state machine

```
Pending ──► HostAccepted ──► Active ──► Completed ──► Released
   │              │              │           │
   ▼              ▼              ▼           ▼
Cancelled      Cancelled     Disputed    DisputeResolved
                                │        DisputeRejected
                                ▼
                            DisputeResolved
                            DisputeRejected
```

#### Instructions

**Admin**

- `initialize_escrow` — admin; creates the `EscrowConfig` account.

**Booking creation**

- `create_booking(check_in, check_out)` — guest opens a booking and escrows USDC; reserves
  calendar days via `BookingDays` (per-month occupancy stored as a `u32` bitmask).
- `create_booking_cross_year(check_in, check_out)` — same as above for stays that span
  two calendar years (reserves days across two `BookingDays` accounts).

**Host response**

- `host_accept_booking` — host accepts a pending booking (`Pending` → `HostAccepted`).
- `host_reject_booking` / `host_reject_booking_cross_year` — host rejects a pending booking;
  the guest receives a full refund, the booking is cancelled and the reserved days are
  released.

**Lifecycle transitions (permissionless)**

- `booking_starts` — anyone can call once the check-in timestamp is reached; transitions
  `HostAccepted` → `Active` and locks the guest's active booking slot via CPI to
  `stayke-core`. No funds are moved.
- `booking_completes` — anyone can call once the check-out timestamp is reached; transitions
  `Active` → `Completed`. Records `updated_at` as the start of the 24 h dispute window and
  the review period. No funds are moved.

**Fund settlement**

- `release_funds` — permissionless; settles a completed stay once the 24 h dispute window
  elapses. Deducts the platform fee (`fee_bps` from `GlobalConfig`), sends the host share,
  and closes the escrow token account (rent returned to the caller). Increments the guest's
  `completed_stays` and the host's `hosted_stays` via CPI to `stayke-core`.

**Cancellations**

- `guest_cancel_booking` / `guest_cancel_booking_cross_year` — guest cancels before
  check-in. Outside the 72 h cancellation window the guest receives a full refund. Inside
  the window the refund is split: 60 % to the guest, 75 % of the remainder to the host,
  and the rest to the platform vault (amounts derived as the remainder so the three shares
  always sum exactly to `total_price`). The guest's `client_cancellations` counter is
  incremented via CPI.
- `host_cancel_booking` / `host_cancel_booking_cross_year` — host cancels before check-in.
  The guest always receives a full refund. Inside the 72 h cancellation window the host's
  guarantee deposit is slashed by 10 % (capped at the available deposit) and paid to the
  guest via CPI to `stayke-treasury`; outside the window there is no slash. The host's
  `host_cancellations` counter is incremented via CPI.

**Expiration**

- `expire_booking` / `expire_booking_cross_year` — permissionless; expires a `Pending`
  booking that has not been acted on for over 24 h. Returns the full escrowed amount to
  the guest and releases the reserved days.

**Reviews**

- `host_review(score)` — host rates the guest (1–5) after the booking reaches `Completed`,
  `Released`, `DisputeResolved`, or `DisputeRejected` status. Updates the guest's
  `ReputationProfile` via CPI.
- `guest_review(score)` — guest rates the host and the listing (1–5) after the same
  terminal statuses. Updates the host's `ReputationProfile` and the listing's
  `total_reviews` / `rating` via CPI.


**CPI endpoints for `stayke-disputes`**

- `cpi_update_booking_status(status)` — allows `stayke-disputes` to flip the booking
  status.
- `cpi_resolve_dispute_transfer(host_share_bps, rejected)` — allows `stayke-disputes`
  to split escrowed funds between host and platform when a dispute resolves.

#### State

| Account | Fields |
| --- | --- |
| `Booking` | `guest`, `host`, `property`, `check_in`, `check_out`, `total_price`, `host_review`, `guest_review`, `status` (`BookingStatus`), `escrow_bump`, `updated_at`, `bump` |
| `BookingDays` | `occupied_days: [u32; 12]`, `year`, `bump` |
| `EscrowConfig` | `authority`, `is_initialized`, `bump` |

#### BookingStatus enum

`Pending`, `HostAccepted`, `Active`, `Completed`, `Released`, `Cancelled`, `Disputed`,
`DisputeResolved`, `DisputeRejected`.

#### Cancellation policy (MVP constants)

| Constant | Value | Description |
| --- | --- | --- |
| `CANCELLATION_WINDOW_HOURS` | `72` | Hours before check-in inside which the split policy applies |
| `CANCELLATION_REFUND_PERCENTAGE` | `60` | % of `total_price` refunded to the guest inside the window |
| `CANCELLATION_HOST_SHARE_PERCENTAGE` | `75` | % of the post-refund remainder paid to the host inside the window |
| `HOST_CANCELLATION_PENALTY_PERCENTAGE` | `10` | % of the host's deposit slashed on late host cancellation |

### `stayke-disputes`

The **dispute resolution** program. Handles disputes over bookings and applies penalties.

- `initialize_config` — admin; creates dispute config.
- `open_dispute(reason)` — a party opens a dispute (`DisputeReason`: property not as
  described, host unreachable, guest damaged property, guest broke rules, other).
- `resolve_dispute(host_share_bps, rejected)` — resolves the dispute and splits the escrow
  between host and platform via CPI to `stayke-escrow`.
- `close_dispute` — closes a resolved dispute.
- `penalize_user(severity)` — applies a penalty (forfeits guarantee via treasury).

State: `Dispute` (booking, initiator, guilty, reason, status, timestamps).

### `stayke-treasury`

The **guarantee vault** program. Holds USDC guarantee deposits and the platform fee vault,
and executes penalty transfers.

- `initialize_treasury` — admin; creates `TreasuryConfig` (authority, `treasury_vault`,
  treasury PDA bump, `global_config` reference).
- `deposit_guarantee(amount)` / `withdraw_guarantee(amount)` — users lock / release their
  guarantee deposit into the vault.
- `cpi_penalize_transfer(amount)` — CPI endpoint used by `stayke-disputes` to move funds when
  a user is penalized.

> Lending and staking instructions (`lend`, `withdraw_from_lending`, `stake`) are declared
> as **placeholders** in the source and are not enabled yet.

---
