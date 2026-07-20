/** Ambient declarations for dependencies that ship no types. */
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
