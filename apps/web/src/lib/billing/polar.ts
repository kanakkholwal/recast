import { Polar } from "@polar-sh/sdk";
import { serverEnv } from "$lib/env/server";

/**
 * Lazy Polar SDK client. Throws if POLAR_ACCESS_TOKEN isn't set so misconfig
 * surfaces at the request handler boundary rather than at module load.
 */

let cached: Polar | null = null;

export function getPolarClient(): Polar {
	if (cached) return cached;
	const { POLAR_ACCESS_TOKEN, POLAR_SERVER } = serverEnv();
	if (!POLAR_ACCESS_TOKEN) {
		throw new Error(
			"POLAR_ACCESS_TOKEN is not set. Add it to .env (use a sandbox token while testing).",
		);
	}
	cached = new Polar({
		accessToken: POLAR_ACCESS_TOKEN,
		server: POLAR_SERVER,
	});
	return cached;
}

export function tryGetPolarClient(): Polar | null {
	if (!serverEnv().POLAR_ACCESS_TOKEN) return null;
	return getPolarClient();
}
