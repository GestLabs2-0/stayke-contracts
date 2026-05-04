import {
	appendTransactionMessageInstructions,
	assertIsFullySignedTransaction,
	assertIsSendableTransaction,
	assertIsSignature,
	assertIsTransactionWithBlockhashLifetime,
	assertIsTransactionWithinSizeLimit,
	compileTransaction,
	createTransactionMessage,
	getBase64EncodedWireTransaction,
	KeyPairSigner,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import {
	findConfigPda,
	getInitializeConfigInstructionAsync,
} from "@generated/stayke-core";
import { SolanaRpcType } from "../connection";
import { confirmTx } from "../utils";

export async function initializeConfig(
	connection: SolanaRpcType,
	payer: KeyPairSigner
	//   args: any
) {
	const configPda = await findConfigPda();

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
		(tx) => compileTransaction(tx)
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
