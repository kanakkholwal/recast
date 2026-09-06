import { build } from "esbuild";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { describe, expect, it } from "vitest";

/**
 * Bundle-size gates for REQUIREMENTS.md §3 — the only budget rows checkable
 * without a browser. Sizes are gzipped KB of a minified, tree-shaken bundle.
 */

const PKG_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const DESKTOP_BUDGET_GZ_KB = 80;
const WORKER_BUDGET_GZ_KB = 220;

/** Bundle `code` as a virtual entry and return its gzipped size in KB. */
async function bundleGzKb(code: string): Promise<number> {
	const result = await build({
		stdin: { contents: code, resolveDir: PKG_ROOT, loader: "ts" },
		bundle: true,
		format: "esm",
		platform: "browser",
		target: "es2022",
		minify: true,
		treeShaking: true,
		write: false,
		logLevel: "silent",
	});
	const out = result.outputFiles?.[0];
	if (!out) throw new Error("esbuild produced no output");
	return gzipSync(Buffer.from(out.contents)).byteLength / 1024;
}

describe("bundle budgets (REQUIREMENTS.md §3)", () => {
	it("the desktop surface stays under budget", async () => {
		const kb = await bundleGzKb(`
			import { getFrameCache, MediaError, isUnsupportedContainer } from './src/index';
			import { MediabunnyVideoSource } from './src/playback/index';
			console.log(getFrameCache, MediaError, isUnsupportedContainer, MediabunnyVideoSource);
		`);
		expect(kb).toBeLessThanOrEqual(DESKTOP_BUDGET_GZ_KB);
	});

	it("the barrel is tree-shakable", async () => {
		// Two regressions: MediaBunny re-exported from the barrel, and no `sideEffects: false`; either made one import cost 61 KB.
		const kb = await bundleGzKb(`
			import { MediaError } from './src/index';
			console.log(MediaError);
		`);
		expect(kb).toBeLessThan(5);
	});

	it("the playback subpath does not pull the conversion pipeline", async () => {
		const kb = await bundleGzKb(`
			import { MediabunnyVideoSource } from './src/playback/index';
			console.log(MediabunnyVideoSource);
		`);
		expect(kb).toBeLessThan(20);
	});

	it("the conversion worker surface stays under budget", async () => {
		// apps/web's client.ts is types-only and spawns the worker lazily, so this never blocks first paint.
		const kb = await bundleGzKb(`
			import { handlers, runConversion, outputFormatFor } from './src/index';
			console.log(handlers, runConversion, outputFormatFor);
		`);
		expect(kb).toBeLessThanOrEqual(WORKER_BUDGET_GZ_KB);
	});
});
