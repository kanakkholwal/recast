// docvia generates `docvia-env.d.ts` (the `virtual:docvia/source` module types)
// at the project root, which SvelteKit's generated tsconfig does not `include`.
// Pull it in from here — the same bootstrap `svelte-kit sync` does for `$types`.
// It is written by `docvia build` (wired into `pnpm check`) and by the dev
// server, so a fresh clone materializes it on the first `pnpm dev` or `check`.
/// <reference path="../docvia-env.d.ts" />

// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// Shape of the object `handleError` returns and `$page.error` exposes.
		// `errorId` correlates a user-facing message with the full stack logged
		// server-side, so support can find the log line without leaking internals.
		interface Error {
			message: string;
			errorId?: string;
		}
		// interface Locals {}
		// interface PageData {}
		// Shallow-routing state. `/playground` swaps its drop surface for the
		// editor on one route, so Back returns to the picker.
		interface PageState {
			playgroundEditing?: boolean;
		}
		// interface Platform {}
	}

	// Injected by Vite `define` — the running web build version, used as an
	// analytics super-property.
	const __APP_VERSION__: string;
}

export {};
