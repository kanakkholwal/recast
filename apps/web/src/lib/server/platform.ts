import { AsyncLocalStorage } from "node:async_hooks";

// Module-level cache is safe: the Hyperdrive binding is identical for a Worker's life (Cloudflare's postgres.js example does the same).
let current: App.Platform | undefined;

export function setRequestPlatform(platform: App.Platform | undefined): void {
	if (platform) current = platform;
}

export function getRequestPlatform(): App.Platform | undefined {
	return current;
}

/** Per-request slot: Workers forbid reusing I/O objects (sockets) across requests, so the db client lives here. */
export interface RequestScope {
	db?: unknown;
}

const scopes = new AsyncLocalStorage<RequestScope>();

export function runInRequestScope<T>(fn: () => T): T {
	return scopes.run({}, fn);
}

export function getRequestScope(): RequestScope | undefined {
	return scopes.getStore();
}
