// gifenc ships no type declarations; declare the small surface we use.
declare module "gifenc" {
	export interface GifEncoderInstance {
		writeFrame(
			index: Uint8Array | number[],
			width: number,
			height: number,
			opts?: { palette?: number[][]; delay?: number; repeat?: number },
		): void;
		finish(): void;
		bytes(): Uint8Array;
	}
	export function GIFEncoder(): GifEncoderInstance;
	export function quantize(
		rgba: Uint8Array | Uint8ClampedArray,
		maxColors: number,
		opts?: unknown,
	): number[][];
	export function applyPalette(
		rgba: Uint8Array | Uint8ClampedArray,
		palette: number[][],
		format?: string,
	): Uint8Array;
}
