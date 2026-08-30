// docvia writes `docvia-env.d.ts` at the root, which SvelteKit's generated tsconfig does not include.

/// <reference path="../docvia-env.d.ts" />

// See https://svelte.dev/docs/kit/types#app.d.ts for these interfaces.
declare global {
	namespace App {
		// The shape `handleError` returns and `$page.error` exposes; `errorId` correlates it with the server-side stack.
		interface Error {
			message: string;
			errorId?: string;
		}
		// Shallow routing: /playground swaps its drop surface for the editor on one route, so Back returns to the picker.
		interface PageState {
			playgroundEditing?: boolean;
		}
		// interface Platform {}
	}

	// Injected by Vite `define`: the running web build version, used as an analytics super-property.
	const __APP_VERSION__: string;
}

export {};
