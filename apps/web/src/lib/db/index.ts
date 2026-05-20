import { env } from "$env/dynamic/private";
import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import * as schema from "./schema";

/**
 * Lazy Postgres client. We don't open a connection at module load so the dev
 * server still boots when DATABASE_URL is missing — the auth UI placeholder
 * keeps working without persistence until you wire a real database.
 *
 * `prepare: false` matches Neon / pgbouncer's transaction-pooled mode; safe
 * default for any Postgres host. Drop it once you switch to a dedicated
 * connection pool.
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

/** Returns the singleton db if env is configured, otherwise null. Use this
 *  from request handlers that should 503 gracefully instead of throwing. */
export function tryGetDb(): Db | null {
	if (!env.DATABASE_URL) return null;
	return getDb();
}

export { schema };
