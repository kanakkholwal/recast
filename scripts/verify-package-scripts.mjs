#!/usr/bin/env node
// Every workspace package must declare a real `check` script. `pnpm --filter`
// silently SKIPS packages without one, so a missing script reads as a pass —
// which is how ~16k LOC across 8 packages went unverified.

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packagesDir = join(root, "packages");

const failures = [];

for (const name of readdirSync(packagesDir)) {
	const manifestPath = join(packagesDir, name, "package.json");
	if (!existsSync(manifestPath)) continue;

	const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
	const check = manifest.scripts?.check;

	if (!check) {
		failures.push(`${manifest.name ?? name}: no "check" script`);
		continue;
	}
	// `exit 0` is the shape this guard exists to catch: it makes `turbo check`
	// report green over a package nothing has ever type-checked.
	if (/^\s*exit\s+0\s*$/.test(check)) {
		failures.push(`${manifest.name ?? name}: "check" is a no-op (${check})`);
	}
}

if (failures.length > 0) {
	console.error("Packages without a real typecheck:\n");
	for (const failure of failures) console.error(`  - ${failure}`);
	console.error(
		"\nAdd a tsconfig.json and either `tsc --noEmit -p tsconfig.json` or" +
			" `svelte-check --tsconfig ./tsconfig.json --threshold error`.",
	);
	process.exit(1);
}

console.log(`All ${readdirSync(packagesDir).length} packages declare a real check script.`);
