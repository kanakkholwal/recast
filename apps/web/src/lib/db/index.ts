import { env } from "$env/dynamic/private";
import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import * as schema from "./schema";

/**
 * Postgres client — lazy on first call, cached afterward. Throws if
 * `DATABASE_URL` isn't set; auth and the API surface are mandatory now, so
 * misconfig should fail loudly rather than silently degrade.
 *
 * `prepare: false` is compatible with Neon / pgbouncer's transaction-pooled
 * mode. Drop it on a dedicated pool if you switch hosts.
 */

type Db = ReturnType<typeof drizzle<typeof schema>>;

let cached: Db | null = null;

export function getDb(): Db {
	if (cached) return cached;
	const url = env.DATABASE_URL;
	if (!url) {
		throw new Error(
			"DATABASE_URL is not set. Copy .env.example to .env and configure a Postgres URL.",
		);
	}
	const client = postgres(url, { prepare: false });
	cached = drizzle(client, { schema });
	return cached;
}

export { schema };
