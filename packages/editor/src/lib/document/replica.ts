/**
 * The webview's replica of a v3 project document. The core's copy is the truth
 * and the sequencer; this copy applies the same ops with the same code, so the
 * two cannot drift, and a whole editor state becomes one op batch here rather
 * than a JSON blob on the wire.
 */

import { isEmptyPatch, shallowPatch } from "@recast/engine";
import type { EditorRenderState } from "../editor/render-state";
import type {
	CommitResult,
	DocumentChanged,
	DocumentDriver,
	DocumentOp,
	ReplicaDocument,
} from "./types";

export interface ReplicaOptions {
	driver: DocumentDriver;
	path: string;
	/** The engine's `ProjectDocument.parse`. */
	parse: (text: string) => ReplicaDocument;
}

/** The target an op writes: id plus attribute for a set, the element for everything else. */
function targetOf(op: DocumentOp): string {
	switch (op.op) {
		case "set":
			return `${op.id}@${op.attr}`;
		case "setText":
			return `${op.id}@#text`;
		case "insert":
			return `${op.parent}@#children`;
		default:
			return op.id;
	}
}

/** Our ops whose target the other writer's batch also wrote; ours land last, so these are the ones that overrode. */
export function overlapping(ours: DocumentOp[], theirs: DocumentOp[]): DocumentOp[] {
	const taken = new Set(theirs.map(targetOf));
	return ours.filter((op) => taken.has(targetOf(op)));
}

export class DocumentReplica {
	#driver: DocumentDriver;
	#path: string;
	#parse: (text: string) => ReplicaDocument;
	#doc: ReplicaDocument;
	#seq: number;
	#hash: string;
	/** The state as last committed, compound fields by identity, so the next commit sends only what moved. */
	#lastState: Record<string, unknown> | null = null;
	#disposed = false;

	private constructor(opts: ReplicaOptions, doc: ReplicaDocument, seq: number, hash: string) {
		this.#driver = opts.driver;
		this.#path = opts.path;
		this.#parse = opts.parse;
		this.#doc = doc;
		this.#seq = seq;
		this.#hash = hash;
	}

	static async open(opts: ReplicaOptions): Promise<DocumentReplica> {
		const snapshot = await opts.driver.show(opts.path);
		const doc = opts.parse(snapshot.text);
		return new DocumentReplica(opts, doc, snapshot.seq, snapshot.hash);
	}

	get path() {
		return this.#path;
	}
	get seq() {
		return this.#seq;
	}
	get hash() {
		return this.#hash;
	}

	/** The editor state the document currently describes. Also resets the patch base: what the caller adopts next
	 *  is what the next commit diffs against. */
	renderState(): Partial<EditorRenderState> {
		const state = JSON.parse(this.#doc.renderState()) as Partial<EditorRenderState>;
		this.#lastState = state as Record<string, unknown>;
		return state;
	}

	/** The ops for `state`: a patch of what moved since the last commit when there is one, the whole state otherwise. */
	#opsFor(state: Partial<EditorRenderState>): DocumentOp[] {
		const next = state as Record<string, unknown>;
		if (this.#lastState) {
			const patch = shallowPatch(this.#lastState, next);
			if (isEmptyPatch(patch)) return [];
			const ops = JSON.parse(this.#doc.opsForPatch(JSON.stringify(patch))) as DocumentOp[];
			this.#lastState = next;
			return ops;
		}
		const ops = JSON.parse(this.#doc.opsForState(JSON.stringify(state))) as DocumentOp[];
		this.#lastState = next;
		return ops;
	}

	/**
	 * Commits the editor's whole state as one batch. On a stale answer the
	 * missed ops are merged into the replica first and ours are retried on top
	 * (an id-level rebase); if they still cannot land, the other writer's
	 * version stands and the caller adopts it.
	 */
	async commit(state: Partial<EditorRenderState>): Promise<CommitResult> {
		this.#assertLive();
		const ops = this.#opsFor(state);
		if (ops.length === 0) return { status: "clean" };

		const first = await this.#driver.apply(this.#path, ops, this.#seq);
		if (first.result === "applied") {
			this.#landed(ops, first.seq, first.hash);
			return { status: "applied", seq: first.seq, ops: ops.length };
		}
		const overlaps = first.since ? overlapping(ops, first.since) : [];
		await this.#catchUp(first.since ?? null, first.seq, first.hash);

		let retry: Awaited<ReturnType<DocumentDriver["apply"]>>;
		try {
			retry = await this.#driver.apply(this.#path, ops, this.#seq);
		} catch (err) {
			return { status: "conflict", seq: this.#seq, reason: String(err) };
		}
		if (retry.result === "applied") {
			this.#landed(ops, retry.seq, retry.hash);
			return { status: "rebased", seq: retry.seq, ops: ops.length, overlaps };
		}
		await this.#catchUp(retry.since ?? null, retry.seq, retry.hash);
		return { status: "conflict", seq: this.#seq, reason: "the document kept moving" };
	}

	/** Folds in another writer's batches. Returns the new state to adopt, or null when nothing was missed. */
	async pull(): Promise<Partial<EditorRenderState> | null> {
		this.#assertLive();
		const missed = await this.#driver.since(this.#path, this.#seq);
		if (missed.seq === this.#seq) return null;
		await this.#catchUp(missed.ops, missed.seq, missed.hash);
		return this.renderState();
	}

	/** Checkpoint now. */
	flush(): Promise<void> {
		return this.#driver.flush(this.#path);
	}

	/** Whether a change event is news to this replica (its own applies advance `seq` before the event arrives). */
	isNews(event: DocumentChanged): boolean {
		return event.path === this.#path && event.seq > this.#seq;
	}

	dispose() {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#doc.free();
	}

	#landed(ops: DocumentOp[], seq: number, hash: string) {
		this.#doc.apply(JSON.stringify(ops));
		this.#seq = seq;
		this.#hash = hash;
		if (this.#doc.hash() !== hash) {
			// Both sides run the same code, so this means a replica bug; a resync is cheaper than guessing.
			void this.#resync();
		}
	}

	async #catchUp(since: DocumentOp[] | null, seq: number, hash: string) {
		if (since) {
			try {
				this.#doc.apply(JSON.stringify(since));
				this.#seq = seq;
				this.#hash = hash;
				if (this.#doc.hash() === hash) return;
			} catch {
				// Fall through to the full read.
			}
		}
		await this.#resync();
	}

	async #resync() {
		const snapshot = await this.#driver.show(this.#path);
		const next = this.#parse(snapshot.text);
		this.#doc.free();
		this.#doc = next;
		this.#seq = snapshot.seq;
		this.#hash = snapshot.hash;
	}

	#assertLive() {
		if (this.#disposed) throw new Error("document replica is disposed");
	}
}
