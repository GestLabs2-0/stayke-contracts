import { createSolanaRpc } from "@solana/kit";

export function createConnection(args: { cluster?: string; rpcUrl?: string }) {
	if (args.rpcUrl) {
		return createSolanaRpc(args.rpcUrl);
	}

	switch (args.cluster) {
		case "devnet":
			return createSolanaRpc("https://api.devnet.solana.com");

		case "mainnet":
			return createSolanaRpc("https://api.mainnet-beta.solana.com");

		case "localnet":
			return createSolanaRpc("http://localhost:8899");

		case "testnet":
			return createSolanaRpc("https://api.testnet.solana.com");

		case "custom":
			return createSolanaRpc("https://api.custom.solana.com");

		default:
			throw new Error("Debes especificar --cluster o --rpc-url");
	}
}

export type SolanaRpcType = ReturnType<typeof createSolanaRpc>;
