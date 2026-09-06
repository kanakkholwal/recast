// See https://svelte.dev/docs/kit/types#app.d.ts for these interfaces.
declare global {
	namespace App {}
	declare const __VERSION__: string;
}

// `gifenc` ships JS only, so this shim treats the import as `any` and svelte-check accepts the workspace traversal.
declare module "gifenc";

export {};
