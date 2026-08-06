/**
 * The editor's single listener for agent activity. Hosts install a driver
 * (`setAgentSessionDriver`); every surface then reads this store rather than
 * touching Tauri, so the web build compiles against the same module with no
 * driver and stays permanently idle.
 */

import type { EditorStore } from "../../stores/editor-store.svelte";
import type { EditorRenderState } from "../editor/render-state";
import { planReconcile } from "./reconcile";
import {
	type AgentActivity,
	type AgentSessionDriver,
	type AgentSessionEvent,
	type AgentSessionMode,
	type AgentSessionSnapshot,
	IDLE_SESSION,
} from "./types";

const MAX_ACTIVITY = 25;

let driver: AgentSessionDriver | null = null;

/** Install the host's transport. Returns a restore fn so tests don't leak. */
export function setAgentSessionDriver(next: AgentSessionDriver | null): () => void {
	const previous = driver;
	driver = next;
	return () => {
		driver = previous;
	};
}

export interface BindOptions {
	store: EditorStore;
	projectPath: string;
	/** Called when adopting would discard unsaved edits. The host decides:
	 *  resolve true to adopt anyway, false to keep the user's version. */
	onConflict?: (incoming: Partial<EditorRenderState>) => Promise<boolean>;
}

function createAgentSession() {
	let session = $state<AgentSessionSnapshot>(IDLE_SESSION);
	let activity = $state.raw<AgentActivity[]>([]);
	let reconciling = $state(false);

	/** True while an agent holds the write-lock: the flag every `inert` and
	 *  `readonly` gate in the editor reads. */
	const active = $derived(session.writer === "agent");

	function note(summary: string) {
		const entry: AgentActivity = {
			id: `${Date.now()}-${activity.length}`,
			atMs: Date.now(),
			summary,
		};
		activity = [...activity, entry].slice(-MAX_ACTIVITY);
	}

	/**
	 * Pull the authoritative edits and fold them into the store. The undo push
	 * happens once per adopt, so a whole agent turn collapses to a single
	 * Ctrl+Z rather than one press per tool call.
	 */
	async function reconcile(opts: BindOptions) {
		const { store, projectPath, onConflict } = opts;
		if (!driver || reconciling) return;
		reconciling = true;
		try {
			const incoming = await driver.readRenderState(projectPath);
			const plan = planReconcile({
				current: store.toRenderState(),
				incoming,
				dirty: store.isDirty,
			});
			if (plan.action === "skip") return;
			if (plan.action === "conflict" && !(await onConflict?.(incoming))) {
				note("Agent changes not applied — you have unsaved edits");
				return;
			}
			store.pushUndoState();
			store.loadRenderState(incoming);
			// The store now matches disk exactly, so it is by definition clean.
			// Without this the 30s autosave would write our own adopt back out.
			store.markSaved(Date.now());
		} finally {
			reconciling = false;
		}
	}

	return {
		get mode(): AgentSessionMode | null {
			return driver?.mode ?? null;
		},
		get active() {
			return active;
		},
		get session() {
			return session;
		},
		get activity() {
			return activity;
		},
		/** True when the user can evict the holder. */
		get canTakeOver() {
			return active && typeof driver?.releaseControl === "function";
		},

		async takeOver() {
			await driver?.releaseControl?.();
			note("You took over from the agent");
		},

		/** Subscribe for one project. Returns a cleanup fn for `$effect`. */
		bind(opts: BindOptions): () => void {
			if (!driver) return () => {};
			let disposed = false;
			let unsubscribe: (() => void) | undefined;

			const onEvent = (event: AgentSessionEvent) => {
				if (event.type === "session") {
					session = event.session;
					return;
				}
				if (event.projectPath !== opts.projectPath) return;
				if (event.summary) note(event.summary);
				void reconcile(opts);
			};

			void (async () => {
				const off = await driver!.subscribe(onEvent);
				if (disposed) off();
				else unsubscribe = off;
				try {
					session = await driver!.getSession();
				} catch {
					// A backend that can't answer leaves us idle rather than blocking
					// the editor behind a lock we can't see.
				}
			})();

			return () => {
				disposed = true;
				unsubscribe?.();
				session = IDLE_SESSION;
				activity = [];
			};
		},
	};
}

export const agentSession = createAgentSession();
