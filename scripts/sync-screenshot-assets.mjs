#!/usr/bin/env node
// Copies the screenshot-editor image assets from their single source of truth
// (packages/application/assets/screenshot-assets) into each consuming app's
// static dir. Those static copies are gitignored, so the repo keeps ONE
// committed copy instead of duplicating ~5MB per app. Runs on predev/prebuild.
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = resolve(root, "packages/application/assets/screenshot-assets");
const targets = [
	resolve(root, "apps/web/static/screenshot-assets"),
	resolve(root, "apps/desktop/static/screenshot-assets"),
];

if (!existsSync(source)) {
	console.error(`[sync-screenshot-assets] source missing: ${source}`);
	process.exit(1);
}

for (const target of targets) {
	rmSync(target, { recursive: true, force: true });
	mkdirSync(dirname(target), { recursive: true });
	cpSync(source, target, { recursive: true });
	console.log(`[sync-screenshot-assets] -> ${target}`);
}
