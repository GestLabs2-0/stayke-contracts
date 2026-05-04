import fs from "fs";
import { createKeyPairSignerFromBytes, KeyPairSigner } from "@solana/kit";

export async function loadKeypair(path: string): Promise<KeyPairSigner> {
	const raw = fs.readFileSync(path, "utf-8");
	const secret = Uint8Array.from(JSON.parse(raw));
	return await createKeyPairSignerFromBytes(secret)
		.then((keypair) => keypair)
		.catch((error: unknown) => {
			console.log("Error creating keypair in Solana Service: ", error);
			throw error;
		});
}
