# Initialize Contracts — CLI de inicialización de Stayke

Herramienta para desplegar las cuentas de configuración de los programas Stayke en una
red Solana (localnet, devnet, mainnet). Cada programa tiene una instrucción `initialize`
que crea sus cuentas PDA on-chain. Esta CLI envuelve esas instrucciones en una
transacción firmada.

## Requisitos previos

- **Node.js** ≥ 18
- **Yarn** (este proyecto usa Yarn workspaces)
- **Solana CLI** (para generar keypairs y fondear cuentas)
- Un keypair con SOL suficiente en el cluster destino:
  - devnet: ~0.5 SOL cubre las tres inicializaciones holgadamente
  - localnet: `solana airdrop 10 <TU_DIRECCION>` al inicio
- **USDC mint address** (requerido por `stayke-config` y `stayke-treasury`)

### USDC mint por cluster

| Cluster  | USDC Mint                                                            |
| -------- | -------------------------------------------------------------------- |
| devnet   | `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`                     |
| mainnet  | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`                    |

## Instalación

```bash
cd tools/initializeContracts
yarn install
```

## Uso general

```bash
yarn start -- \
  --program <stayke-core|stayke-config|stayke-treasury> \
  --keypair <ruta/al/keypair.json> \
  [flags globales]
```

Alternativa directa con `tsx`:

```bash
npx tsx src/index.ts --program stayke-core --keypair ./keypair.json ...
```

---

## Flags globales

Estos flags aplican a los tres programas.

| Flag         | Requerido | Descripción                                     |
| ------------ | --------- | ----------------------------------------------- |
| `--program`  | ✅        | Programa a inicializar: `stayke-core`, `stayke-config` o `stayke-treasury` |
| `--keypair`  | ✅        | Ruta al archivo JSON del keypair que firma y paga la transacción |
| `--cluster`  | ❌        | Cluster: `devnet` (default implícito vía Anchor.toml), `mainnet`, `localnet`, `testnet` |
| `--rpc-url`  | ❌        | URL de RPC custom (pisa `--cluster`)            |

Si no pasás `--cluster` ni `--rpc-url`, la conexión por defecto apunta a devnet
(la URL configurada en `Anchor.toml` bajo `[provider]`).

---

## 1. stayke-core

Inicializa la cuenta `Config` del programa `stayke_core`. Esta cuenta es la
configuración mínima que necesita el core para operar. La instrucción no recibe
argumentos adicionales — solo crea el PDA con seeds `["config"]`.

### Flags específicos

Ninguno. Solo requiere los [flags globales](#flags-globales).

### Ejemplo

```bash
yarn start -- \
  --program stayke-core \
  --keypair ./keypair.json \
  --cluster devnet
```

---

## 2. stayke-config

Inicializa la cuenta `GlobalConfig` del programa `stayke_config`. Esta cuenta
es la configuración compartida que usan el core, escrow, disputes y treasury.
**Crea dos PDAs en una sola instrucción**: `GlobalConfig` y la token account
`platform_vault` (donde se acumulan las fees).

### Flags específicos

| Flag                  | Requerido | Default (devnet) | Descripción |
| --------------------- | --------- | ---------------- | ----------- |
| `--mint-address`      | ✅        | —                | Dirección del mint de USDC |
| `--fee-bps`           | ❌        | `500`            | Fee de plataforma en basis points (5%). Debe ser < 10000 |
| `--minimum-deposit`   | ❌        | `100000`         | Depósito mínimo en la unidad más chica de USDC (0.1 USDC) |
| `--max-operations`    | ❌        | `3`              | Cantidad máxima de operaciones sin stake antes de exigir staking |
| `--core-program`      | ❌        | ver abajo        | Program ID del programa stayke_core |
| `--escrow-program`    | ❌        | ver abajo        | Program ID del programa stayke_escrow |
| `--disputes-program`  | ❌        | ver abajo        | Program ID del programa stayke_disputes |
| `--treasury-program`  | ❌        | ver abajo        | Program ID del programa stayke_treasury |

Los defaults de los program IDs en devnet:

```
--core-program      2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm
--escrow-program    68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a
--disputes-program  89yo4qWuvaQcAPtAcutNB6vht3JwvEwMMLbSwpMM2Czt
--treasury-program  3JE5y7vtjkZkA6s3eRAKorT1eQmgoJQmnVqpy15uUjq8
```

### Qué hace la instrucción

La instrucción `initialize_config` del programa `stayke_config`:

- Crea la cuenta `GlobalConfig` (PDA con seed `["global_config"]`)
- Crea la token account `platform_vault` (PDA con seed `["platform_vault_token"]`,
  autoridad = PDA `["platform_vault"]`)
- Almacena: authority, minimum_deposit, fee_bps, max_operations, y la allowlist
  de programas que pueden recibir CPI (core, escrow, disputes, treasury)

### Ejemplo

```bash
yarn start -- \
  --program stayke-config \
  --keypair ./keypair.json \
  --cluster devnet \
  --mint-address 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --fee-bps 500 \
  --minimum-deposit 100000 \
  --max-operations 3
```

Con program IDs custom (ej: después de redeployar en localnet):

```bash
yarn start -- \
  --program stayke-config \
  --keypair ./keypair.json \
  --cluster devnet \
  --mint-address 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --core-program AbCdEf1234567890AbCdEf1234567890AbCdEf12 \
  --escrow-program BcDeFg2345678901BcDeFg2345678901BcDeFg23 \
  --disputes-program CdEfGh3456789012CdEfGh3456789012CdEfGh34 \
  --treasury-program DeFgHi4567890123DeFgHi4567890123DeFgHi45
```

---

## 3. stayke-treasury

Inicializa la cuenta `Config` del programa `stayke_treasury`. Esta cuenta
almacena la referencia al mint de USDC que usa la tesorería.

### Flags específicos

| Flag             | Requerido | Default | Descripción |
| ---------------- | --------- | ------- | ----------- |
| `--mint-address` | ✅        | —       | Dirección del mint de USDC |

### Ejemplo

```bash
yarn start -- \
  --program stayke-treasury \
  --keypair ./keypair.json \
  --cluster devnet \
  --mint-address 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
```

---

## Orden recomendado de inicialización

El orden lógico para un deploy limpio es:

```
1. stayke-core     → crea la Config del core (semilla de todo)
2. stayke-config   → crea la GlobalConfig con la allowlist de programas
3. stayke-treasury → crea la Config de la tesorería
```

Cada paso es independiente a nivel de transacción (no hay dependencia on-chain
estricta entre ellos), pero el core y la config global son prerequisito conceptual
para que el resto de la lógica de negocio funcione.

---

## Flags por programa — resumen

| Flag                  | core | config | treasury |
| --------------------- | :--: | :----: | :------: |
| `--mint-address`      |  —   |   ✅   |    ✅    |
| `--fee-bps`           |  —   |   ❌   |    —     |
| `--minimum-deposit`   |  —   |   ❌   |    —     |
| `--max-operations`    |  —   |   ❌   |    —     |
| `--core-program`      |  —   |   ❌   |    —     |
| `--escrow-program`    |  —   |   ❌   |    —     |
| `--disputes-program`  |  —   |   ❌   |    —     |
| `--treasury-program`  |  —   |   ❌   |    —     |

✅ = requerido &nbsp;&nbsp; ❌ = opcional &nbsp;&nbsp; — = no aplica

---

## Troubleshooting

### "Error: Debes pasar --mint-address"

`stayke-config` y `stayke-treasury` requieren la dirección del mint de USDC.
Pasala con `--mint-address`.

### "GlobalConfig already exists at ..."

La cuenta GlobalConfig ya fue inicializada en este cluster. Si estás en localnet
y necesitás re-inicializar (porque cambió el layout), cerrá o wipeá la cuenta
manualmente antes de volver a correr el comando.

### "Transaction failed: ..."

La transacción llegó a la red pero falló. Posibles causas:
- La cuenta ya existe (para `stayke-config` vas a ver el error de arriba)
- El programa no está deployado en ese cluster
- Los program IDs del allowlist no coinciden con los programas deployados
- El fee_bps es ≥ 10000 (el programa lo rechaza con `InvalidFeeBps`)

### "Error creating keypair"

El archivo de keypair no es válido o no existe en la ruta especificada.
Verificá que sea un array JSON de 64 bytes (formato estándar de Solana).
