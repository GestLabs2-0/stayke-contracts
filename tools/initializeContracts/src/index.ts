import { parseArgs } from "./config";
import { createConnection } from "./connection";
import { initializeConfig } from "./programs/staykeCore";
import { initializeDisputesConfig } from "./programs/staykeDisputes";
import { initializeEscrowConfig } from "./programs/staykeEscrow";
import { initializeGlobalConfig } from "./programs/staykeGlobalConfig";
import { initializeTreasuryConfig } from "./programs/staykeTreasury";
import { loadKeypair } from "./wallet";

async function main() {
	const args = parseArgs();

	const keypair = await loadKeypair(args.keypair);
	const connection = createConnection(args);

	switch (args.program) {
		case "stayke-core":
		case "core":
			await initializeConfig(connection, keypair);
			break;
		case "stayke-config":
		case "config":
			await initializeGlobalConfig(connection, keypair, args);
			break;
		case "stayke-treasury":
		case "treasury":
			await initializeTreasuryConfig(connection, keypair, args);
			break;
		case "stayke-disputes":
		case "disputes":
			await initializeDisputesConfig(connection, keypair);
			break;
		case "stayke-escrow":
		case "escrow":
			await initializeEscrowConfig(connection, keypair);
			break;

		default:
			throw new Error(`Programa no soportado: ${args.program}`);
	}
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
