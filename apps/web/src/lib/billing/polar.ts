import { env } from "$env/dynamic/private";
import { Polar } from "@polar-sh/sdk";

/**
 * Lazy Polar SDK client. Throws if POLAR_ACCESS_TOKEN isn't set so misconfig
 * surfaces at the request handler boundary rather than at module load.
 */

let cached: Polar | null = null;

export function getPolarClient(): Polar {
	if (cached) return cached;
	const accessToken = env.POLAR_ACCESS_TOKEN;
	if (!accessToken) {
		throw new Error(
			"POLAR_ACCESS_TOKEN is not set. Add it to .env (use a sandbox token while testing).",
		);
	}
	cached = new Polar({
		accessToken,
		server: env.POLAR_SERVER === "production" ? "production" : "sandbox",
	});
	return cached;
}

export function tryGetPolarClient(): Polar | null {
	if (!env.POLAR_ACCESS_TOKEN) return null;
	return getPolarClient();
}
