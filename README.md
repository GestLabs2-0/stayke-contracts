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
| [`stayke-config`](#stayke-config) | `2GM2yLmDtz2Hyb8T5VBftERmiyJ5whKUmv6V4hBjNXMW` | Shared protocol configuration |
| [`stayke-core`](#stayke-core) | `8yHjmyUgA9x4pzftX1cwJt8SnG8iV1zxLjEP77HKc9YP` | Users, listings and reputation |
| [`stayke-escrow`](#stayke-escrow) | `FRXoLmSWKjMBmHz2Wfn2BPV3mcjkWZ2ESMRWUiwjb2iQ` | Booking lifecycle and fund escrow |
| [`stayke-disputes`](#stayke-disputes) | `7SQdT9RxCjsEbap9vCmyVdAURwC7XRJkZtPNSJBcDxRB` | Dispute resolution and penalties |
| [`stayke-treasury`](#stayke-treasury) | `59buEPHFBK4h8LyLE2KtnV1kpaQTyjb82NWt5F9jSuHu` | Guarantee deposits and platform vault |

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
> the signer against this global config, instead of hardcoding program addresses. This is
> not fully wired up yet — only `stayke-treasury` currently references `global_config`.

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

- `initialize_escrow` — admin; initializes escrow config.
- `create_booking(check_in, check_out)` — guest opens a booking; checks calendar availability
  via `BookingDays` (per-month occupancy stored as a `u32` bitmask).
- `host_accept_booking` / `host_reject_booking` — host responds to a pending booking.
- `client_accept_reserve` / `client_reject_reserve` — guest confirms/rejects the reservation.
- `review_completed(score)` — guest rates the stay (0 = none, 1–5 stars).
- `complete_stay` — releases escrowed funds to the host.
- `cpi_update_booking_status` / `cpi_resolve_dispute_transfer` — CPI endpoints used by
  `stayke-disputes` to flip booking status and move funds when a dispute resolves.

State: `Booking` (guest, host, property, deposit, dates, total price, review, status, bumps)
and `BookingDays` (occupied calendar days).

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

## Design notes (from the author)

The original README held open design questions. They are preserved here for context:

- **Global config**: every contract should share the same data instead of duplicating
  config. The cleanest approach is a dedicated global contract (this became `stayke-config`)
  with config accounts in each contract referencing it, so CPI signer checks are consistent
  and program addresses are not hardcoded.
- **Banned user with an active booking**: if a user is banned while a booking is still
  active, should the funds be returned to the client, or should Stayke take a share?

**Security TODO:** modifications from other contracts must not be allowed unless they are
secured beforehand. The recommended path is to route everything through the global config
contract (`stayke-config`) and have each contract hold a reference to it.
