import { assertIsSignature } from "@solana/kit";
import { SolanaRpcType } from "./connection";
export async function confirmTx(connection: SolanaRpcType, signature: string) {
	assertIsSignature(signature);
	for (let i = 0; i < 20; i++) {
		const res = await connection.getSignatureStatuses([signature]).send();

		const status = res.value[0];

		if (status) {
			if (status.err) {
				throw new Error(`Transaction failed: ${JSON.stringify(status.err)}`);
			}

			if (
				status.confirmationStatus === "confirmed" ||
				status.confirmationStatus === "finalized"
			) {
				console.log("✅ Transaction confirmed:", status.confirmationStatus);
				return;
			}
		}

		await new Promise((r) => setTimeout(r, 500));
	}

	throw new Error("Transaction not confirmed in time");
}
