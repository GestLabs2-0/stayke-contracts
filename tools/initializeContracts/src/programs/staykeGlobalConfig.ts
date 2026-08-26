import {
  fetchMaybeGlobalConfig,
  findGlobalConfigPda,
  getInitializeConfigInstructionAsync,
} from "@GestLabs2-0/stayke-config";
import {
  address,
  appendTransactionMessageInstructions,
  assertIsFullySignedTransaction,
  assertIsSendableTransaction,
  assertIsTransactionWithinSizeLimit,
  compileTransaction,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  type KeyPairSigner,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import type { SolanaRpcType } from "../connection";
import { confirmTx } from "../utils";

/** Devnet program IDs from Anchor.toml — used as defaults for the allowed-programs allowlist. */
const DEVNET_PROGRAMS = {
  core: "2u1JrVasLvuGR5s3n84p5yaitHU2PGa8VjWZ7P2Eescm",
  escrow: "68ipZiXiUhsaSYSqEM3619vXgKy5CqFmNE6rYzxrXu6a",
  disputes: "89yo4qWuvaQcAPtAcutNB6vht3JwvEwMMLbSwpMM2Czt",
  treasury: "3JE5y7vtjkZkA6s3eRAKorT1eQmgoJQmnVqpy15uUjq8",
} as const;

/**
 * GlobalConfig layout is adopted via wipe/re-init only (no migrate instruction).
 * If an account already exists at the GlobalConfig PDA, refuse and instruct operators
 * to close/wipe it on localnet before re-initializing.
 */
export async function initializeGlobalConfig(
  connection: SolanaRpcType,
  payer: KeyPairSigner,
  {
    mintAddress,
    feeBps,
    minimumDeposit,
    freeOps,
    maxOperations,
    coreProgram,
    escrowProgram,
    disputesProgram,
    treasuryProgram,
  }: {
    mintAddress?: string;
    feeBps?: number;
    minimumDeposit?: number;
    freeOps?: number;
    maxOperations?: number;
    coreProgram?: string;
    escrowProgram?: string;
    disputesProgram?: string;
    treasuryProgram?: string;
  },
) {
  if (!mintAddress) {
    throw Error("Debes pasar --mint-address");
  }
  const configPda = await findGlobalConfigPda();

  const existing = await fetchMaybeGlobalConfig(connection, configPda[0]);
  if (existing.exists) {
    throw new Error(
      `GlobalConfig already exists at ${configPda[0]}. ` +
        `GlobalConfig layout changes require wipe/re-init (no migrate instruction). ` +
        `Close or wipe the account on localnet, then re-run initialize.`,
    );
  }

  const instruction = await getInitializeConfigInstructionAsync({
    authority: payer,
    globalConfig: configPda[0],
    feeBps: feeBps ?? 500,
    minimumDeposit: minimumDeposit ?? 100000,
    freeOps: freeOps ?? maxOperations ?? 3,
    usdcMint: address(mintAddress),
    core: address(coreProgram ?? DEVNET_PROGRAMS.core),
    escrow: address(escrowProgram ?? DEVNET_PROGRAMS.escrow),
    disputes: address(disputesProgram ?? DEVNET_PROGRAMS.disputes),
    treasury: address(treasuryProgram ?? DEVNET_PROGRAMS.treasury),
  });

  const { value: latestBlockhash } = await connection
    .getLatestBlockhash()
    .send();

  const transaction = pipe(
    createTransactionMessage({ version: 0 }),
    (tx) => setTransactionMessageFeePayerSigner(payer, tx),
    (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    (tx) => appendTransactionMessageInstructions([instruction], tx),
    (tx) => compileTransaction(tx),
  );

  assertIsTransactionWithinSizeLimit(transaction);

  const signatures = await payer.signTransactions([transaction]);
  const signedTx = {
    ...transaction,
    signatures: {
      ...transaction.signatures,
      ...signatures[0],
    },
  };
  assertIsFullySignedTransaction(signedTx);
  assertIsSendableTransaction(signedTx);

  const base64EncodedTx = getBase64EncodedWireTransaction(signedTx);

  const signature = await connection
    .sendTransaction(base64EncodedTx, {
      encoding: "base64",
      maxRetries: 5n,
      preflightCommitment: "confirmed",
    })
    .send();

  console.log("Transaction signature:", signature);
  await confirmTx(connection, signature);
}
