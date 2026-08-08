import {
	address,
	appendTransactionMessageInstructions,
	assertIsFullySignedTransaction,
	assertIsSendableTransaction,
	assertIsTransactionWithinSizeLimit,
	compileTransaction,
	createTransactionMessage,
	getBase64EncodedWireTransaction,
	KeyPairSigner,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import { SolanaRpcType } from "../connection";
import { confirmTx } from "../utils";

import {
	getInitializeConfigInstructionAsync,
	findGlobalConfigPda,
	fetchMaybeGlobalConfig,
} from "@GestLabs2-0/stayke-config";

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
	}: {
		mintAddress?: string;
		feeBps?: number;
		minimumDeposit?: number;
	}
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
				`Close or wipe the account on localnet, then re-run initialize.`
		);
	}

	const instruction = await getInitializeConfigInstructionAsync({
		authority: payer,
		globalConfig: configPda[0],
		feeBps: feeBps ?? 500,
		minimumDeposit: minimumDeposit ?? 100000,
		usdcMint: address(mintAddress),
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
