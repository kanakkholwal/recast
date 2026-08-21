/**
 * Decides what to do when the backend reports that a project's edits changed
 * underneath us. Pure — no store, no transport — so every branch is testable
 * without mounting an editor.
 */

import type { EditorRenderState } from "../editor/render-state";
import { sameRenderState } from "./canonical";

export type ReconcilePlan =
	/** Incoming state already matches ours. Doing nothing is what breaks the
	 *  write→echo→write loop, so this branch must stay first. */
	| { action: "skip"; reason: "identical" }
	/** Safe to adopt: nothing unsaved would be lost. */
	| { action: "apply" }
	/** The user has unsaved edits AND the on-disk state diverged. Adopting
	 *  would silently discard their work, so the host must ask. */
	| { action: "conflict" };

export interface ReconcileInput {
	/** What the store currently holds, via `store.toRenderState()`. */
	current: Partial<EditorRenderState>;
	/** What the backend just wrote to disk. */
	incoming: Partial<EditorRenderState>;
	/** `store.isDirty` — true when the user has uncommitted edits. */
	dirty: boolean;
}

export function planReconcile({ current, incoming, dirty }: ReconcileInput): ReconcilePlan {
	if (sameRenderState(current, incoming)) return { action: "skip", reason: "identical" };
	return dirty ? { action: "conflict" } : { action: "apply" };
}
