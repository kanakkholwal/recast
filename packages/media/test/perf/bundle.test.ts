import { build } from 'esbuild';
import { gzipSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';

/**
 * Bundle-size gates for REQUIREMENTS.md §3. These are the latency-free budget
 * rows, so unlike the rest they can be enforced without a browser.
 */

const DESKTOP_BUDGET_GZ_KB = 80;
const WEB_BUDGET_GZ_KB = 150;

/** Bundle `code` as a virtual entry and return its gzipped size in KB. */
async function bundleGzKb(code: string): Promise<number> {
	const result = await build({
		stdin: { contents: code, resolveDir: new URL('../../', import.meta.url).pathname, loader: 'ts' },
		bundle: true,
		format: 'esm',
		platform: 'browser',
		target: 'es2022',
		minify: true,
		treeShaking: true,
		write: false,
		logLevel: 'silent',
	});
	const out = result.outputFiles?.[0];
	if (!out) throw new Error('esbuild produced no output');
	return gzipSync(Buffer.from(out.contents)).byteLength / 1024;
}

describe('bundle budgets (REQUIREMENTS.md §3)', () => {
	it('the desktop playback surface stays under budget', async () => {
		const kb = await bundleGzKb(`
			import { getFrameCache, MediaError, isUnsupportedContainer } from './src/index';
			console.log(getFrameCache, MediaError, isUnsupportedContainer);
		`);
		expect(kb).toBeLessThanOrEqual(DESKTOP_BUDGET_GZ_KB);
	});

	it('the web conversion surface stays under budget', async () => {
		const kb = await bundleGzKb(`
			import { runConversion, outputFormatFor, handlers } from './src/index';
			console.log(runConversion, outputFormatFor, handlers);
		`);
		expect(kb).toBeLessThanOrEqual(WEB_BUDGET_GZ_KB);
	});

	it('the main barrel does not drag MediaBunny into every consumer', async () => {
		// Regression: MediaBunny used to be re-exported from `src/index.ts`, so a
		// consumer importing only `MediaError` still paid for the whole library.
		const errorOnly = await bundleGzKb(`
			import { MediaError } from './src/index';
			console.log(MediaError);
		`);
		const withMediabunny = await bundleGzKb(`
			import { Input } from './src/mediabunny';
			console.log(Input);
		`);
		expect(errorOnly).toBeLessThan(withMediabunny / 2);
		expect(errorOnly).toBeLessThan(20);
	});
});
