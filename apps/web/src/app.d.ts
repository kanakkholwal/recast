/// <reference path="../docvia-env.d.ts" />

// The reference above pulls in `docvia-env.d.ts` at the root, which SvelteKit's generated tsconfig does not include.
import type { DurableObjectNamespace, KVNamespace } from "@cloudflare/workers-types";

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
		// Cloudflare Workers only; undefined under Vercel/Node and in `vite dev`. `lib/server/platform.ts` degrades to DATABASE_URL when absent.
		interface Platform {
			env?: {
				KV?: KVNamespace;
				DO?: DurableObjectNamespace;
				HYPERDRIVE?: { connectionString: string };
			};
		}
	}

	// Injected by Vite `define`: the running web build version, used as an analytics super-property.
	const __APP_VERSION__: string;
}
