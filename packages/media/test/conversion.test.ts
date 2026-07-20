import { describe, expect, it } from 'vitest';
import { withExtension } from '../src/conversion';

/**
 * Pure-utility tests that exercise the surface area WITHOUT needing the
 * real implementations. When the real `runConversion` lands in PR-B, the
 * smoke and conversion-handler tests land alongside it.
 */
describe('withExtension', () => {
	it('replaces the existing extension', () => {
		expect(withExtension('clip.mp4', 'webm')).toBe('clip.webm');
		expect(withExtension('clip.MOV', 'mp4')).toBe('clip.mp4');
	});

	it('adds an extension when none is present', () => {
		expect(withExtension('clip', 'mp4')).toBe('clip.mp4');
	});

	it('keeps multi-dot filenames intact', () => {
		expect(withExtension('my.clip.v2.mp4', 'webm')).toBe('my.clip.v2.webm');
	});
});
