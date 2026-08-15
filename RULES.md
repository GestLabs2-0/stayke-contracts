# RULES.md
> Single source of truth for coding agents working in this repository.
> Read this file in full before making any changes.

---

## Architecture context

This is an Anchor-based Solana project composed of multiple programs communicating via CPI.
- Programs: `stayke-core`, `stayke-bookings`, `stayke-treasury`, `stayke-disputes`
- Authority source: `GlobalConfig` PDA — all cross-program authority flows through it
- Payment token: USDC (SPL Token)
- Key patterns: CPI with signer seeds, PDA-derived authorities, account constraint validation

---

## Development rules

### CPI security (CRITICAL)
Every CPI call MUST enforce proper authorization. "Providing a signer" is not sufficient —
the signer must be validated as a specific, expected authority.

Required pattern:
- The `authority` account in any CPI handler must be constrained, not just marked `Signer`.
- Use Anchor account constraints (`has_one`, `constraint =`, or `seeds`/`bump`) to verify
  the signer is the expected PDA or keypair — never accept an arbitrary signer.
- If the CPI invokes a privileged instruction (fund transfer, dispute resolution, state mutation),
  the authority must be derived from `GlobalConfig` or the relevant program's PDA.

Bad (DO NOT do this):
```rust
pub authority: Signer<'info>,  // Anyone can pass this
```

Good:
```rust
#[account(
    constraint = authority.key() == global_config.disputes_authority @ StaykeError::Unauthorized
)]
pub authority: Signer<'info>,
```

### General rules
- Never use `unwrap()` or `expect()` in program code — use proper error propagation with custom errors.
- All custom errors must be defined in the program's `errors.rs` and documented with a comment.
- Account discriminators must be verified on deserialization for any cross-program account read.
- No magic numbers — use named constants for seeds, fees, timeouts, and limits.
- If a task works in tests, always handle each instruction in a different file while working on unit tests
---

## Skills

Load the relevant skill(s) before starting any implementation. Multiple skills can be active simultaneously.

| Trigger | Skill |
|---|---|
| Implementing or modifying any Anchor program | `stayke-contracts/skills/anchor.md` |
| Writing or modifying tests (unit or integration) | `stayke-contracts/skills/testing.md` |

If a task touches both (e.g., implementing a feature AND writing its tests), load both skills.

---
## Code quality checks

Run in this exact order after every change. A failure in any step blocks the next.

```bash
# 1. Format check (must pass before linting)
cargo +nightly fmt --all -- --check

# 2. Dependency audit
cargo deny check advisories

# 3. Lint (zero warnings policy)
cargo clippy --all-targets --tests -- -Dwarnings

# 4. Build
anchor build

# 4. Tests
cargo test --all-targets
```

If `fmt` fails: run `cargo +nightly fmt --all` and re-check.
If `clippy` fails: fix all warnings — do not use `#[allow(...)]` without explicit justification in a comment.
If tests fail: do not proceed with further changes until the regression is identified.

---

## Code exploration (CodeGraph)

**Use CodeGraph FIRST** before grep, find, or manual file reading when locating or understanding code.

If a `.codegraph/` directory exists at repo root:
- **MCP tool** (preferred): `codegraph_explore` — returns verbatim source + call paths, including dynamic dispatch.
- **Shell fallback**: `codegraph explore "<symbol or question>"`

If no `.codegraph/` directory exists: init codegraph.
If codegraph could not be initiated, skip it.
