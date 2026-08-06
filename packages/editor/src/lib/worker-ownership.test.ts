import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * Workers are spawned by the HOST APP, never from inside this package: a
 * `new URL('./x-worker', import.meta.url)` here resolves outside the app's
 * root, so the app's dev server has to whitelist it — which fails silently in
 * dev while production bundles fine. Mirrors the same rule in
 * `packages/media/test/worker-resolution.test.ts`.
 */

const LIB = dirname(fileURLToPath(import.meta.url));
const SRC = resolve(LIB, "..");

function sourceFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((entry) => {
		const full = join(dir, entry);
		if (statSync(full).isDirectory()) return sourceFiles(full);
		return /\.(ts|svelte)$/.test(entry) && !/\.test\.ts$/.test(entry) ? [full] : [];
	});
}

const stripComments = (code: string) =>
	code.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");

describe("worker ownership", () => {
	it("never spawns a worker from inside the package", () => {
		const offenders = sourceFiles(SRC).filter((f) => {
			const code = stripComments(readFileSync(f, "utf8"));
			return /new Worker\(/.test(code) || /new URL\([^)]*import\.meta\.url/.test(code);
		});
		expect(offenders, "spawning belongs to the host app").toEqual([]);
	});

	it("exposes a start function for every worker body", () => {
		// Static, not a dynamic import: these modules touch worker globals.
		const bodies = {
			"playback/render-worker.ts": "startRenderWorker",
			"timeline/filmstrip-worker.ts": "startFilmstripWorker",
			"cursor/smoothing-worker.ts": "startSmoothingWorker",
			"export/export-render.worker.ts": "startExportRenderWorker",
		};
		for (const [rel, fn] of Object.entries(bodies)) {
			const code = readFileSync(join(LIB, rel), "utf8");
			expect(code, `${rel} must export ${fn}`).toContain(`export function ${fn}(`);
		}
	});
});
