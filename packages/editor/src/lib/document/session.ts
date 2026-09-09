/**
 * Binds a store to a document replica for one open project: mirrors edits to
 * the core as op batches, folds other writers' batches back into the store,
 * and checkpoints on demand. Transport-free; the host installs a driver.
 */

import type { EditorRenderState } from "../editor/render-state";
import { getDocumentDriver } from "./driver";
import { DocumentReplica } from "./replica";
import type {
	CommitResult,
	DocumentDriver,
	DocumentInvalid,
	DocumentOp,
	ReplicaDocument,
} from "./types";

/** The slice of the editor store the session touches; structural so tests can fake it. */
export interface SessionStore {
	readonly isDirty: boolean;
	toRenderState(): Partial<EditorRenderState>;
	loadRenderState(state: Partial<EditorRenderState>): void;
	pushUndoState(): void;
	markSaved(savedAtUnixMs: number): void;
}

export interface SessionOptions {
	store: SessionStore;
	projectPath: string;
	/** The engine's `ProjectDocument.parse`. */
	parse: (text: string) => ReplicaDocument;
	/** Edits kept landing under ours: the other writer's version now stands in the store. */
	onConflict?: (reason: string) => void;
	/** Both sides wrote the same properties; ours won. The user should hear what the other writer lost. */
	onOverlap?: (overlaps: DocumentOp[]) => void;
	/** The file on disk was edited into something the core refused. */
	onInvalid?: (event: DocumentInvalid) => void;
	onError?: (context: string, err: unknown) => void;
	driver?: DocumentDriver;
	/** How long after the last edit the mirror runs. */
	mirrorDelayMs?: number;
	now?: () => number;
}

export const DEFAULT_MIRROR_DELAY_MS = 750;

export class DocumentSession {
	#replica: DocumentReplica;
	#opts: Required<Pick<SessionOptions, "store" | "now" | "mirrorDelayMs">> & SessionOptions;
	#unsubscribe: (() => void) | undefined;
	#unsubscribeInvalid: (() => void) | undefined;
	#timer: ReturnType<typeof setTimeout> | null = null;
	#inFlight: Promise<CommitResult> | null = null;
	/** A change event arrived mid-commit (possibly our own, before seq advanced); settle it once the flight lands. */
	#pendingRemote = false;
	#disposed = false;

	private constructor(replica: DocumentReplica, opts: SessionOptions) {
		this.#replica = replica;
		this.#opts = {
			...opts,
			now: opts.now ?? Date.now,
			mirrorDelayMs: opts.mirrorDelayMs ?? DEFAULT_MIRROR_DELAY_MS,
		};
	}

	/** Null when no driver is installed: the store stays the only copy, as on the web. */
	static async open(opts: SessionOptions): Promise<DocumentSession | null> {
		const driver = opts.driver ?? getDocumentDriver();
		if (!driver) return null;
		const replica = await DocumentReplica.open({
			driver,
			path: opts.projectPath,
			parse: opts.parse,
		});
		const session = new DocumentSession(replica, opts);
		session.#unsubscribe = await driver.subscribe((event) => {
			if (session.#disposed || !replica.isNews(event)) return;
			if (session.#inFlight) session.#pendingRemote = true;
			else void session.#adoptRemote();
		});
		if (opts.onInvalid && driver.subscribeInvalid) {
			const onInvalid = opts.onInvalid;
			session.#unsubscribeInvalid = await driver.subscribeInvalid((event) => {
				if (!session.#disposed && event.path === opts.projectPath) onInvalid(event);
			});
		}
		return session;
	}

	get seq() {
		return this.#replica.seq;
	}
	get hash() {
		return this.#replica.hash;
	}

	/** Mirror after the debounce; each edit pushes the deadline back. */
	scheduleCommit() {
		if (this.#disposed) return;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			this.#timer = null;
			void this.commit();
		}, this.#opts.mirrorDelayMs);
	}

	/** Commits the store's state now. Concurrent calls share one flight. */
	commit(): Promise<CommitResult> {
		if (this.#inFlight) return this.#inFlight;
		// Assigned before the body runs: a driver may emit synchronously inside apply, and that event must see the flight.
		const flight = Promise.resolve().then(() => this.#commitNow());
		this.#inFlight = flight.finally(() => {
			this.#inFlight = null;
			if (this.#pendingRemote) {
				this.#pendingRemote = false;
				void this.#adoptRemote();
			}
		});
		return this.#inFlight;
	}

	/** Commit, then checkpoint the file: the explicit save. */
	async save(): Promise<CommitResult> {
		const result = await this.commit();
		await this.#replica.flush();
		return result;
	}

	flush(): Promise<void> {
		return this.#replica.flush();
	}

	async dispose() {
		if (this.#disposed) return;
		if (this.#timer) clearTimeout(this.#timer);
		this.#timer = null;
		try {
			if (this.#opts.store.isDirty) await this.commit();
			await this.#replica.flush();
		} catch (err) {
			this.#opts.onError?.("dispose", err);
		}
		this.#disposed = true;
		this.#unsubscribe?.();
		this.#unsubscribeInvalid?.();
		this.#replica.dispose();
	}

	async #commitNow(): Promise<CommitResult> {
		if (this.#disposed) return { status: "clean" };
		const { store } = this.#opts;
		let result: CommitResult;
		try {
			result = await this.#replica.commit(store.toRenderState());
		} catch (err) {
			this.#opts.onError?.("commit", err);
			throw err;
		}
		if (result.status === "clean") return result;
		if (result.status === "applied") {
			store.markSaved(this.#opts.now());
			// An edit made during the flight is hidden behind the cleared dirty flag; one follow-up pass finds it (and is free when there is none).
			this.scheduleCommit();
			return result;
		}
		// The document moved under us: what the replica now holds is the merged truth.
		this.#adopt(this.#replica.renderState());
		if (result.status === "conflict") this.#opts.onConflict?.(result.reason);
		else if (result.overlaps.length > 0) this.#opts.onOverlap?.(result.overlaps);
		return result;
	}

	async #adoptRemote() {
		try {
			// Unsaved edits are mirrored first, so the rebase inside commit merges the two rather than one replacing the other.
			if (this.#opts.store.isDirty) {
				await this.commit();
				return;
			}
			const next = await this.#replica.pull();
			if (next) this.#adopt(next);
		} catch (err) {
			this.#opts.onError?.("pull", err);
		}
	}

	#adopt(state: Partial<EditorRenderState>) {
		const { store } = this.#opts;
		store.pushUndoState();
		store.loadRenderState(state);
		store.markSaved(this.#opts.now());
	}
}
