import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { serverEnv } from "$lib/env/server";
import { getRequestPlatform } from "$lib/server/platform";
import * as schema from "./schema";

/**
 * Postgres client — lazy on first call, cached afterward. Prefers the Cloudflare
 * Hyperdrive binding (pooled + edge-local to the Worker) and falls back to
 * `DATABASE_URL` directly outside Workers: local `vite dev`, the Vercel adapter
 * path, and CI's `drizzle-kit` scripts never see a `platform.env`.
 *
 * `prepare: false` is required by Hyperdrive's connection pooling and also
 * happens to suit Neon's pgbouncer transaction-pooled mode. `fetch_types: false`
 * skips postgres.js's pg_catalog introspection query, which is the query that
 * fails first once a Worker-held raw TCP socket to Neon has gone stale. `max: 5`
 * caps concurrent sockets per isolate, since Workers has no true connection pool.
 */

type Db = ReturnType<typeof drizzle<typeof schema>>;

let cached: Db | null = null;

export function getDb(): Db {
	if (cached) return cached;
	const connectionString =
		getRequestPlatform()?.env?.HYPERDRIVE?.connectionString ?? serverEnv().DATABASE_URL;
	const client = postgres(connectionString, { prepare: false, fetch_types: false, max: 5 });
	cached = drizzle(client, { schema });
	return cached;
}

export { schema };
