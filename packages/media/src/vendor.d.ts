/**
 * Ambient declarations for dependencies that ship no types.
 *
 * `gifenc` is plain JS with no bundled `.d.ts` and no `@types/gifenc` on npm.
 * The desktop app papered over this in its own `untyped.d.ts`, which meant
 * the media package itself never type-checked in isolation. Declaring it
 * here lets `pnpm --filter @recast/media check` run for real.
 */
declare module 'gifenc' {
	export function GIFEncoder(): {
		writeFrame(
			index: Uint8Array,
			width: number,
			height: number,
			options?: Record<string, unknown>,
		): void;
		finish(): void;
		bytes(): Uint8Array;
		bytesView(): Uint8Array;
	};
	export function quantize(
		rgba: Uint8Array | Uint8ClampedArray,
		maxColors: number,
		options?: Record<string, unknown>,
	): number[][];
	export function applyPalette(
		rgba: Uint8Array | Uint8ClampedArray,
		palette: number[][],
		format?: string,
	): Uint8Array;
}
