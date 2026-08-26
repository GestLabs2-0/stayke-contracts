# Tools

## update_programs.sh

Script to deploy, update, and manage programs on Solana clusters.

```bash
./update_programs.sh                   # devnet, full pipeline
./update_programs.sh --net localnet    # localnet
./update_programs.sh --extend-only     # only extend program accounts, useful for diagnostics
./update_programs.sh --deploy-only     # deploy only (assumes account is already extended)
./update_programs.sh --skip-idl        # deploy without updating IDL accounts
```

---

## initializeContracts CLI

CLI tool to initialize on-chain configurations and PDAs for Stayke protocol contracts on Solana clusters (localnet, devnet, mainnet).

### Running the Tool

From the repository root:

```bash
pnpm --filter initialize-contracts start -- <options>
```

Or from `tools/initializeContracts`:

```bash
cd tools/initializeContracts
pnpm start -- <options>
```

### CLI Arguments & Options

| Option | Required | Description |
|---|---|---|
| `--keypair <path>` | **Yes** | Path to the authority/payer keypair JSON file. |
| `--program <name>` | **Yes** | Target program to initialize (`stayke-config`, `stayke-treasury`, `stayke-core`, `stayke-disputes`, `stayke-escrow`). |
| `--cluster <cluster>` | No* | Solana cluster (`devnet`, `mainnet`, `localnet`, `testnet`, `custom`). *Required if `--rpc-url` is omitted. |
| `--rpc-url <url>` | No* | Custom Solana RPC endpoint URL. *Required if `--cluster` is omitted. |
| `--mint-address <pubkey>` | Conditional | USDC / Token mint address. Required for `stayke-config` and `stayke-treasury`. |
| `--fee-bps <number>` | No | Platform fee basis points for `stayke-config` (default: `500`). |
| `--minimum-deposit <number>` | No | Minimum deposit amount for `stayke-config` (default: `100000`). |
| `--free-ops <number>` | No | Free operations before deposit is required for `stayke-config` (default: `3`). |
| `--core-program <pubkey>` | No | Custom program ID for stayke-core allowlist in `stayke-config`. |
| `--escrow-program <pubkey>` | No | Custom program ID for stayke-escrow allowlist in `stayke-config`. |
| `--disputes-program <pubkey>` | No | Custom program ID for stayke-disputes allowlist in `stayke-config`. |
| `--treasury-program <pubkey>` | No | Custom program ID for stayke-treasury allowlist in `stayke-config`. |

---

### Program Initializations & Examples

#### 1. Initialize Global Config (`stayke-config`)

Initializes the `GlobalConfig` PDA (`[b"global_config"]`) storing protocol-wide settings, allowed program IDs, and the platform fee vault.

```bash
pnpm --filter initialize-contracts start -- \
  --keypair ~/.config/solana/id.json \
  --cluster devnet \
  --program stayke-config \
  --mint-address 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --fee-bps 500 \
  --minimum-deposit 100000 \
  --free-ops 3
```

#### 2. Initialize Treasury (`stayke-treasury`)

Initializes the `TreasuryConfig` PDA (`[b"config"]`), treasury authority PDA (`[b"treasury"]`), and the treasury vault token account (`[b"treasury_vault"]`).

```bash
pnpm --filter initialize-contracts start -- \
  --keypair ~/.config/solana/id.json \
  --cluster devnet \
  --program stayke-treasury \
  --mint-address 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
```

#### 3. Initialize Core (`stayke-core`)

Initializes the `ConfigAcc` PDA (`[b"config"]`) for the core protocol.

```bash
pnpm --filter initialize-contracts start -- \
  --keypair ~/.config/solana/id.json \
  --cluster devnet \
  --program stayke-core
```

#### 4. Initialize Disputes (`stayke-disputes`)

Initializes the `DisputeConfig` PDA (`[b"dispute_config"]`), registering the authority as the first dispute admin.

```bash
pnpm --filter initialize-contracts start -- \
  --keypair ~/.config/solana/id.json \
  --cluster devnet \
  --program stayke-disputes
```

#### 5. Initialize Escrow (`stayke-escrow`)

Initializes the `EscrowConfig` PDA (`[b"escrow_config"]`) linked to the global config.

```bash
pnpm --filter initialize-contracts start -- \
  --keypair ~/.config/solana/id.json \
  --cluster devnet \
  --program stayke-escrow
```
