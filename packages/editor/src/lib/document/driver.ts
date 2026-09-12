/**
 * The one slot holding the host's document transport. No driver means no
 * replica: a host without a v3 core (the web editor today) keeps its store as
 * the only copy and nothing here throws.
 */

import type { DocumentDriver } from "./types";

let driver: DocumentDriver | null = null;

/** Install the host's transport. Returns a restore fn so tests don't leak. */
export function setDocumentDriver(next: DocumentDriver | null): () => void {
	const previous = driver;
	driver = next;
	return () => {
		driver = previous;
	};
}

export function getDocumentDriver(): DocumentDriver | null {
	return driver;
}
