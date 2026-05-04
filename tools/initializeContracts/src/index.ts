import { parseArgs } from "./config";
import { createConnection } from "./connection";
import { initializeConfig } from "./programs/staykeCore";
import { initializeGlobalConfig } from "./programs/staykeGlobalConfig";
import { initializeTreasuryConfig } from "./programs/staykeTreasury";
import { loadKeypair } from "./wallet";

async function main() {
	const args = parseArgs();

	const keypair = await loadKeypair(args.keypair);
	const connection = createConnection(args);

	switch (args.program) {
		case "stayke-core":
			await initializeConfig(connection, keypair);
			break;
		case "stayke-config":
			await initializeGlobalConfig(connection, keypair, args);
			break;
		case "stayke-treasury":
			await initializeTreasuryConfig(connection, keypair, args);
			break;

		default:
			throw new Error(`Programa no soportado: ${args.program}`);
	}
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
