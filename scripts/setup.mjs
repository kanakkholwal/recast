#!/usr/bin/env node
// Cross-platform dispatcher for the local development setup.
//
// This is a convenience wrapper for re-runs once Node.js is already
// installed. On a brand-new machine (no Node yet), run the OS-specific
// bootstrap script directly instead:
//
//   Windows:        powershell -ExecutionPolicy Bypass -File scripts/setup.ps1
//   macOS / Linux:  bash scripts/setup.sh
//
// Any arguments are forwarded to the underlying script, e.g.
//   pnpm setup:ffmpeg -- --skip-build

import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const forwarded = process.argv.slice(2);

let command;
let args;

if (process.platform === "win32") {
  command = "powershell";
  args = [
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    join(scriptDir, "setup.ps1"),
    ...forwarded,
  ];
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
