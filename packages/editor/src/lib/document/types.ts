/**
 * Contracts for the v3 document replica. Transport-free: the desktop host
 * drives them over Tauri IPC, a web host would over HTTP, and the replica
 * never learns which.
 */

/** One edit to the document, addressed by id or kind path. Mirrors `recast_project::Op` (a wire contract). */
export type DocumentOp =
	| { op: "set"; id: string; attr: string; value: string | null }
	| { op: "setText"; id: string; text: string }
	| { op: "insert"; parent: string; index: number; node: DocumentNodeSpec }
	| { op: "remove"; id: string }
	| { op: "move"; id: string; parent: string; index: number };

export interface DocumentNodeSpec {
	kind: string;
	attrs?: Record<string, string>;
	text?: string;
	children?: DocumentNodeSpec[];
}

export interface DocumentSnapshot {
	text: string;
	hash: string;
	seq: number;
}

/** What the core answered to an apply. A stale answer is a fact, not a failure: it carries the ops the writer missed. */
export type DocumentApplyOutcome =
	| { result: "applied"; seq: number; hash: string; warnings?: number }
	| { result: "stale"; seq: number; hash: string; since?: DocumentOp[] };

export interface DocumentChanged {
	path: string;
	seq: number;
	hash: string;
}

export interface DocumentDriver {
	show(path: string): Promise<DocumentSnapshot>;
	apply(path: string, ops: DocumentOp[], expectSeq: number): Promise<DocumentApplyOutcome>;
	/** `ops` is null when the core's ring no longer reaches `seq`. */
	since(
		path: string,
		seq: number,
	): Promise<{ ops: DocumentOp[] | null; seq: number; hash: string }>;
	/** Checkpoint now rather than after the debounce. */
	flush(path: string): Promise<void>;
	/** Fires after every writer's batch. Resolves to an unsubscribe fn. */
	subscribe(sink: (event: DocumentChanged) => void): Promise<() => void>;
}

/** The in-webview document (the engine's wasm `ProjectDocument`). Structural so tests can fake it. */
export interface ReplicaDocument {
	free(): void;
	text(): string;
	hash(): string;
	apply(opsJson: string): number;
	renderState(): string;
	opsForState(stateJson: string): string;
}

export type CommitResult =
	/** The state already matched the document. */
	| { status: "clean" }
	/** The batch landed as `seq`. */
	| { status: "applied"; seq: number; ops: number }
	/** Another writer moved the document first; its ops were merged in and ours landed on top. The host must adopt the replica's state. */
	| { status: "rebased"; seq: number; ops: number }
	/** Ours could not land on the moved document. The replica holds the other writer's version; the host must adopt it. */
	| { status: "conflict"; seq: number; reason: string };
