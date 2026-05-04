import type { AnchorIdl } from "@codama/nodes-from-anchor";

import { rootNodeFromAnchor } from "@codama/nodes-from-anchor";
import { renderVisitor } from "@codama/renderers-js";
import { createFromRoot } from "codama";
import { promises as fs } from "fs";
import path from "path";

const dirname = import.meta.dirname;

const loadAnchorIDLs = async () => {
	const basePath = path.join(dirname, "../../../target/idl");
	const dirPath = path.join(basePath);

	try {
		const files = await fs.readdir(dirPath);
		const jsonFiles = files.filter((file) => file.endsWith(".json"));

		const filePaths = jsonFiles.map((file) => path.join(dirPath, file));
		const fileContents = await Promise.all(
			filePaths.map((filePath) => fs.readFile(filePath, "utf-8"))
		);
		return fileContents.map((content) => JSON.parse(content) as AnchorIdl);
	} catch (err) {
		if (err instanceof Error && "code" in err && err.code === "ENOENT") {
			throw new Error(`Failed to load IDL: ${dirPath} does not exist`);
		}
		throw err;
	}
};

async function main() {
	const idls = await loadAnchorIDLs();

	const programs = idls.map((idl) => {
		let name = "UnknownProgram";
		if ("metadata" in idl && idl.metadata && "name" in idl.metadata) {
			name = idl.metadata.name;
		}

		return {
			codama: createFromRoot(rootNodeFromAnchor(idl)),
			name: name,
		};
	});

	await Promise.all(
		programs.map(async (program) => {
			const generatedPath = path.join(
				dirname,
				"../../../packages",
				program.name.toLowerCase()
			);
			await program.codama
				.accept(
					renderVisitor(generatedPath, {
						syncPackageJson: true,
						generatedFolder: "src/",
					})
				)
				.then(() => {
					console.log(generatedPath, ": Done");
				})
				.catch((err: unknown) => {
					console.log(err);
				});

			const packageJsonPath = path.join(generatedPath, "package.json");

			const packageFileRead = await fs
				.readFile(packageJsonPath, "utf-8")
				.then((val) => val)
				.catch((error) => console.log(error));

			const pkg = JSON.parse(packageFileRead as string);

			pkg.name = `@generated/${program.name.toLowerCase().replace(/_/g, "-")}`;

			await fs.writeFile(packageJsonPath, JSON.stringify(pkg, null, 2));
		})
	);
}

await main();
