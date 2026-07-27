import { build } from 'esbuild';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

/**
 * The decode worker is spawned by the HOST APP, not by this package: a
 * `new URL('./worker.ts', import.meta.url)` here resolves outside the app's
 * root, which the app's dev server then has to whitelist. That failed silently
 * in dev while production bundled fine, so it's pinned here.
 */

const PKG_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SOURCE = resolve(PKG_ROOT, 'src/playback/source.ts');

describe('decode worker ownership', () => {
	it('never spawns a worker from inside the package', () => {
		// Comments stripped: the docstring names the pattern it forbids.
		const code = readFileSync(SOURCE, 'utf8')
			.replace(/\/\*[\s\S]*?\*\//g, '')
			.replace(/\/\/.*$/gm, '');
		expect(code).not.toMatch(/new URL\(.+import\.meta\.url/);
		expect(code).not.toMatch(/new Worker\(/);
	});

	it('exposes the worker body for a host app to mount', async () => {
		const mod = await import('../src/playback/worker');
		expect(typeof mod.startMediabunnyWorker).toBe('function');
	});

	it('bundles the worker standalone, with no import reaching outside the package', async () => {
		const result = await build({
			entryPoints: [resolve(PKG_ROOT, 'src/playback/worker.ts')],
			bundle: true,
			format: 'esm',
			platform: 'browser',
			target: 'es2022',
			write: false,
			metafile: true,
			logLevel: 'silent',
		});
		const inputs = Object.keys(result.metafile?.inputs ?? {});
		expect(inputs.length).toBeGreaterThan(0);
		const strays = inputs.filter((p) => /(^|[/\\])apps[/\\]/.test(p));
		expect(strays, 'worker must not import from an app').toEqual([]);
	});
});
