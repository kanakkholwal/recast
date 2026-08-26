// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
	declare const __VERSION__: string;
}

// `gifenc` ships JS only (no .d.ts). Used by `@recast/media/encoders` (PR-B);
// the shim treats the import as `any` so svelte-check doesn't reject the
// workspace traversal. Function signatures we rely on: `GIFEncoder()`,
// `enc.writeFrame(...)`, `enc.finish()`, `enc.bytes()`, `quantize(rgba, n)`,
// `applyPalette(rgba, palette)`.
declare module "gifenc";

export {};
