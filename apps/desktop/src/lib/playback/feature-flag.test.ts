import { describe, expect, it } from 'vitest';
import { isMediabunnyPreviewEnabled } from './feature-flag';

/**
 * Pure-function tests for the playback feature flag. Browser envs are out
 * of scope for vitest (the config is `environment: 'node'`), so we stub
 * `window` per-case via a tiny helper.
 */
function withLocation(search: string, fn: () => void): void {
	const original = (globalThis as { window?: unknown }).window;
	(globalThis as { window?: unknown }).window = { location: { search } };
	try {
		fn();
	} finally {
		(globalThis as { window?: unknown }).window = original;
	}
}

describe('isMediabunnyPreviewEnabled', () => {
	it('defaults to ON (MediaBunny) when no flag is set', () => {
		withLocation('', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(true);
		});
	});

	it('defaults to ON when the search is empty', () => {
		withLocation('', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(true);
		});
	});

	it('returns ON when `?mbPreview=1` is set (legacy alias)', () => {
		withLocation('?mbPreview=1', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(true);
		});
	});

	it('returns OFF when `?mbPreview=0` is set (legacy alias)', () => {
		withLocation('?mbPreview=0', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(false);
		});
	});

	it('returns ON when `?useLegacyPreview=0` is set', () => {
		withLocation('?useLegacyPreview=0', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(true);
		});
	});

	it('returns OFF when `?useLegacyPreview=1` is set (escape hatch)', () => {
		withLocation('?useLegacyPreview=1', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(false);
		});
	});

	it('honors `useLegacyPreview` over `mbPreview` (last write wins)', () => {
		withLocation('?mbPreview=1&useLegacyPreview=1', () => {
			expect(isMediabunnyPreviewEnabled()).toBe(false);
		});
	});

	it('treats `no` and `off` as falsy for the mbPreview alias', () => {
		for (const v of ['no', 'off', 'false', '']) {
			withLocation(`?mbPreview=${v}`, () => {
				expect(isMediabunnyPreviewEnabled()).toBe(false);
			});
		}
	});
});