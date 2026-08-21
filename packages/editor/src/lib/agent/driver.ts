/**
 * The one slot holding the host's agent transport. Kept apart from the session
 * store so the branch store can read the same driver without importing a rune
 * module into a plain one.
 */

import type { AgentSessionDriver } from "./types";

let driver: AgentSessionDriver | null = null;

/** Install the host's transport. Returns a restore fn so tests don't leak. */
export function setAgentSessionDriver(next: AgentSessionDriver | null): () => void {
	const previous = driver;
	driver = next;
	return () => {
		driver = previous;
	};
}

export function getAgentSessionDriver(): AgentSessionDriver | null {
	return driver;
}
