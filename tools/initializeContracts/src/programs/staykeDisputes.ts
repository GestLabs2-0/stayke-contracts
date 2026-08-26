import {
  fetchMaybeDisputeConfig,
  findDisputeConfigPda,
  getInitializeConfigInstructionAsync,
} from "@GestLabs2-0/stayke-disputes";
import {
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

export async function initializeDisputesConfig(
  connection: SolanaRpcType,
  payer: KeyPairSigner,
) {
  const configPda = await findDisputeConfigPda();

  const existing = await fetchMaybeDisputeConfig(connection, configPda[0]);
  if (existing.exists) {
    throw new Error(
      `DisputeConfig already exists at ${configPda[0]}. Disputes is already initialized.`,
    );
  }

  const instruction = await getInitializeConfigInstructionAsync({
    authority: payer,
    config: configPda[0],
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
