export type CLIArgs = {
	keypair: string;
	cluster?: string;
	rpcUrl?: string;
	program: string;
	mintAddress?: string;
	feeBps?: number;
	minimumDeposit?: number;
};

export function parseArgs(): CLIArgs {
	const args = process.argv.slice(2);

	const getArg = (name: string) => {
		const index = args.indexOf(name);
		if (index === -1) return undefined;
		return args[index + 1];
	};

	const keypair = getArg("--keypair");
	const program = getArg("--program");

	if (!keypair) throw new Error("Debes pasar --keypair");
	if (!program) throw new Error("Debes pasar --program");

	const minimumDeposit = getArg("--minimum-deposit");
	const feeBps = getArg("--fee-bps");

	return {
		keypair,
		program,
		cluster: getArg("--cluster"),
		rpcUrl: getArg("--rpc-url"),
		mintAddress: getArg("--mint-address"),
		feeBps: feeBps ? parseInt(feeBps) : undefined,
		minimumDeposit: minimumDeposit ? parseInt(minimumDeposit) : undefined,
	};
}
