import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { serverEnv } from "$lib/env/server";
import * as schema from "./schema";

/**
 * Postgres client — lazy on first call, cached afterward. Env validation lives
 * in `$lib/env/server`; `DATABASE_URL` is required, so a missing or malformed
 * value is caught before this function ever runs.
 *
 * `prepare: false` is compatible with Neon / pgbouncer's transaction-pooled
 * mode. Drop it on a dedicated pool if you switch hosts.
 */

type Db = ReturnType<typeof drizzle<typeof schema>>;

let cached: Db | null = null;

export function getDb(): Db {
	if (cached) return cached;
	const client = postgres(serverEnv().DATABASE_URL, { prepare: false });
	cached = drizzle(client, { schema });
	return cached;
}

export { schema };
