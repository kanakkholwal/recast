#!/usr/bin/env node
// Convenience wrapper for re-runs once Node exists; on a fresh machine run scripts/setup.ps1 or scripts/setup.sh directly. Args are forwarded.

import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const forwarded = process.argv.slice(2);

let command;
let args;

if (process.platform === "win32") {
	command = "powershell";
	args = ["-ExecutionPolicy", "Bypass", "-File", join(scriptDir, "setup.ps1"), ...forwarded];
} else {
	command = "bash";
	args = [join(scriptDir, "setup.sh"), ...forwarded];
}

const child = spawn(command, args, { stdio: "inherit" });
child.on("exit", (code) => process.exit(code ?? 1));
child.on("error", (err) => {
	console.error(`Failed to launch ${command}: ${err.message}`);
	process.exit(1);
});
