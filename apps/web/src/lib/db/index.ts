import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";
import { serverEnv } from "$lib/env/server";
import { getRequestPlatform, getRequestScope } from "$lib/server/platform";
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

function connect(): Db {
	const connectionString =
		getRequestPlatform()?.env?.HYPERDRIVE?.connectionString ?? serverEnv().DATABASE_URL;
	const client = postgres(connectionString, { prepare: false, fetch_types: false, max: 5 });
	return drizzle(client, { schema });
}

/**
 * Decision (architecture): on Workers the client is per request, not per isolate.
 * A socket opened in one request throws "Cannot perform I/O on behalf of a different
 * request" when another request writes to it, which broke every OAuth callback.
 * Node/Vercel and scripts (no platform or no request scope) keep the singleton.
 */
export function getDb(): Db {
	const scope = getRequestScope();
	if (scope && getRequestPlatform()) {
		scope.db ??= connect();
		return scope.db as Db;
	}
	cached ??= connect();
	return cached;
}

/** Stable handle for long-lived singletons (the auth adapter) that must resolve the current request's client on every call. */
export const lazyDb: Db = new Proxy({} as Db, {
	get(_target, prop) {
		const db = getDb();
		const value = Reflect.get(db, prop, db);
		return typeof value === "function" ? value.bind(db) : value;
	},
});

export { schema };
