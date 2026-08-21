/**
 * Branch contracts: edits an agent has proposed but not yet applied.
 *
 * Transport-free, like the rest of `lib/agent`. The desktop host wires these to
 * Tauri commands; the editor never learns which.
 */

import type { EditorRenderState } from "../editor/render-state";

/**
 * Discriminants of Rust's `render::ops::Op`. Serialized into stored journals,
 * so this list is a wire contract: `ops.rs`'s
 * `every_variant_tag_is_accounted_for` asserts the same names.
 */
export const EDIT_OP_TAGS = [
	"replace",
	"trim",
	"cutAdd",
	"cutRemove",
	"zoomAdd",
	"zoomRemove",
	"splitPointAdd",
	"splitPointRemove",
	"speedSet",
	"speedRemove",
	"annotationAdd",
	"annotationUpdate",
	"annotationRemove",
	"animationAdd",
	"animationRemove",
	"set",
] as const;

export type EditOpTag = (typeof EDIT_OP_TAGS)[number];

/** One proposed edit. Mirrors Rust's `Op`, tagged by `op`. */
export type EditOp =
	| { op: "replace"; state: EditorRenderState }
	| { op: "trim"; start: number; end: number }
	| { op: "cutAdd"; start: number; end: number }
	| { op: "cutRemove"; index?: number; start?: number; end?: number }
	| { op: "zoomAdd"; region: Record<string, unknown> }
	| { op: "zoomRemove"; index: number }
	| { op: "splitPointAdd"; at: number }
	| { op: "splitPointRemove"; at: number }
	| { op: "speedSet"; segmentStart: number; rate: number }
	| { op: "speedRemove"; segmentStart: number }
	| { op: "annotationAdd"; annotation: Record<string, unknown> }
	| { op: "annotationUpdate"; id: string; patch: Record<string, unknown> }
	| { op: "annotationRemove"; id: string }
	| { op: "animationAdd"; start: number; animIn?: unknown; animOut?: unknown }
	| { op: "animationRemove"; start: number }
	| { op: "set"; field: string; value: unknown };

/** A branch without its ops, for list views. */
export interface BranchSummary {
	id: string;
	author: string;
	label: string | null;
	/** Hex content hash of the render state the branch forked from. */
	base: string;
	/** Sequence number of the newest entry; `0` on an empty branch. */
	seq: number;
	ops: number;
	createdAtMs: number;
	updatedAtMs: number;
}

/** One leaf that differs between the project and a branch. */
export interface FieldChange {
	/** Dotted path, e.g. `cuts.0.end` or `audioSettings.volume`. */
	field: string;
	/** `null` when the branch adds this leaf. */
	before: unknown;
	/** `null` when the branch removes it. */
	after: unknown;
}

export interface AppendReport {
	seq: number;
	/** `false` when the idem key was already on the branch. */
	recorded: boolean;
	compacted: boolean;
}

export interface ApplyReport {
	changes: number;
}

/**
 * Host-supplied branch transport. A host without one leaves the review panel
 * hidden rather than erroring.
 */
export interface BranchDriver {
	list(projectPath: string): Promise<BranchSummary[]>;
	diff(projectPath: string, branch: string): Promise<FieldChange[]>;
	materialize(projectPath: string, branch: string): Promise<Partial<EditorRenderState>>;
	discard(projectPath: string, branch: string): Promise<void>;
	apply(projectPath: string, branch: string, writerId: string): Promise<ApplyReport>;
	truncate(projectPath: string, branch: string, seq: number): Promise<BranchSummary>;
}

/** Split a dotted path into the collection it addresses and the rest. */
export function changeGroup(field: string): string {
	const [head] = field.split(".");
	return head || field;
}

/** Human-facing label for one change row. */
export function describeChange(change: FieldChange): "added" | "removed" | "changed" {
	if (change.before === null || change.before === undefined) return "added";
	if (change.after === null || change.after === undefined) return "removed";
	return "changed";
}

/**
 * Group changes by their top-level collection, preserving path order within
 * each group so a review list reads the way the state is laid out.
 */
export function groupChanges(changes: readonly FieldChange[]): Map<string, FieldChange[]> {
	const groups = new Map<string, FieldChange[]>();
	for (const change of changes) {
		const key = changeGroup(change.field);
		const bucket = groups.get(key);
		if (bucket) bucket.push(change);
		else groups.set(key, [change]);
	}
	return groups;
}
