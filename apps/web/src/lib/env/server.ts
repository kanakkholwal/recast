import { building, dev } from "$app/environment";
import { env as rawEnv } from "$env/dynamic/private";
import { serverEnvSchema, type ServerEnv } from "./schema";

/**
 * Validated server-side env, single source of truth for everything secret.
 *
 * Imported via a getter (not as a top-level constant) so SvelteKit's
 * `vite build` step can load this module without `DATABASE_URL` /
 * `BETTER_AUTH_SECRET` being set — those only need to exist at runtime.
 * `hooks.server.ts` calls `getServerEnv()` once during boot so the very first
 * request still surfaces a missing-var error eagerly, instead of failing
 * mid-handler.
 *
 * Never import this module from anything that can be bundled for the browser;
 * SvelteKit blocks `$env/dynamic/private` from client bundles, so you'd get a
 * build error if you tried.
 */

let cached: ServerEnv | null = null;

export function getServerEnv(): ServerEnv {
	if (cached) return cached;
	
	if (dev){
		console.info("[NODE_ENV]:",rawEnv.NODE_ENV);
	}

	// Skip validation while Vite is collecting the prerender manifest — env
	// isn't available there and prerendered pages don't need it anyway.
	if (building) {
		cached = serverEnvSchema.parse({
			DATABASE_URL: "postgres://build-time-stub",
			BETTER_AUTH_SECRET: "build-time-stub-secret-not-used-at-runtime-xxxxxxxx",
		});
		return cached;
	}

	const result = serverEnvSchema.safeParse(rawEnv);
	if (!result.success) {
		const flat = result.error.issues
			.map((i) => `  • ${i.path.join(".") || "(root)"}: ${i.message}`)
			.join("\n");
		throw new Error(
			`Invalid environment variables — fix .env then restart:\n${flat}`,
		);
	}
	cached = result.data;
	return cached;
}

/**
 * Sugar for the common case `getServerEnv().FOO`. Looks nice at call sites:
 *
 *   import { serverEnv } from "$lib/env/server";
 *   const url = serverEnv().DATABASE_URL;
 */
export const serverEnv = getServerEnv;
