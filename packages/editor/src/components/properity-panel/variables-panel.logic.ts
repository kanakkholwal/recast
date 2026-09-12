import type { VarSpec } from "../../lib/editor/render-state";

/** Variables under one `path`, in the order the document declared them. */
export interface VarGroup {
	/** The declared `path`, or "" for the ungrouped ones that lead the panel. */
	path: string;
	vars: VarSpec[];
}

/**
 * Groups by `path`, ungrouped first, then paths in the order their first
 * variable appears. Document order is the author's ordering, so nothing sorts.
 */
export function groupVariables(vars: VarSpec[]): VarGroup[] {
	const groups: VarGroup[] = [];
	for (const spec of vars) {
		const path = spec.path?.trim() ?? "";
		const existing = groups.find((g) => g.path === path);
		if (existing) existing.vars.push(spec);
		else groups.push({ path, vars: [spec] });
	}
	return groups.sort((a, b) => Number(a.path !== "") - Number(b.path !== ""));
}

/** The two halves of a `vec2`, which the document spells `x,y`. */
export function readVec2(value: string): [number, number] {
	const [x, y] = value.split(",");
	return [Number.parseFloat(x ?? "") || 0, Number.parseFloat(y ?? "") || 0];
}

export function writeVec2(x: number, y: number): string {
	return `${trimNumber(x)},${trimNumber(y)}`;
}

/** Up to six decimals, trailing zeros gone, matching how the document spells a number. */
export function trimNumber(value: number): string {
	if (!Number.isFinite(value)) return "0";
	if (Number.isInteger(value)) return String(value);
	return String(Number.parseFloat(value.toFixed(6)));
}

export function readNumber(value: string, fallback = 0): number {
	const parsed = Number.parseFloat(value);
	return Number.isFinite(parsed) ? parsed : fallback;
}

/**
 * Decimal places a field should show: none for an int, else enough for the
 * declared step, capped at three so a field never jitters wider than its column.
 */
export function decimalsFor(spec: VarSpec): number {
	if (spec.type === "int") return 0;
	const step = spec.step ?? 1;
	if (!Number.isFinite(step) || step <= 0) return 0;
	// Counted by rounding rather than by the printed string, which goes exponential below 1e-6.
	for (let places = 0; places < 3; places++) {
		if (Number(step.toFixed(places)) === step) return places;
	}
	return 3;
}

/** Options a `select` offers; a declaration with none still shows its own value rather than an empty menu. */
export function selectOptions(spec: VarSpec): { value: string; label: string }[] {
	const declared = (spec.options ?? []).map((o) => o.trim()).filter(Boolean);
	const all = declared.includes(spec.value) ? declared : [...declared, spec.value];
	return all.filter(Boolean).map((value) => ({ value, label: value }));
}
