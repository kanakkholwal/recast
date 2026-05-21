import { env as rawPublicEnv } from "$env/dynamic/public";
import { publicEnvSchema, type PublicEnv } from "./schema";

/**
 * Validated public env. Safe to import from both server and client — every
 * value here is shipped to the browser, so do not add secrets.
 *
 * Validation happens once and is cached; the resulting object is a plain
 * snapshot, not a live proxy, so don't mutate `process.env` and expect
 * `publicEnv()` to pick it up.
 */

let cached: PublicEnv | null = null;

export function getPublicEnv(): PublicEnv {
	if (cached) return cached;
	const result = publicEnvSchema.safeParse(rawPublicEnv);
	if (!result.success) {
		const flat = result.error.issues
			.map((i) => `  • ${i.path.join(".") || "(root)"}: ${i.message}`)
			.join("\n");
		throw new Error(
			`Invalid PUBLIC_* environment variables:\n${flat}`,
		);
	}
	cached = result.data;
	return cached;
}

export const publicEnv = getPublicEnv;
