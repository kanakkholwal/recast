/**
 * The edit path's unit of change: the top-level render-state fields that differ
 * between two snapshots. Compound fields compare by reference, so the store has
 * to hand out memoised snapshots for this to be cheap; primitives by value.
 */
export type StatePatch = Record<string, unknown>;

/** Fields present in `next` whose value (primitive) or identity (object) changed; `null` marks a removed field. */
export function shallowPatch(
	prev: Record<string, unknown>,
	next: Record<string, unknown>,
): StatePatch {
	const patch: StatePatch = {};
	for (const key of Object.keys(next)) {
		const value = next[key];
		if (value === undefined) continue;
		if (!Object.is(prev[key], value)) patch[key] = value;
	}
	for (const key of Object.keys(prev)) {
		if (prev[key] !== undefined && next[key] === undefined) patch[key] = null;
	}
	return patch;
}

export function isEmptyPatch(patch: StatePatch): boolean {
	for (const _ in patch) return false;
	return true;
}
