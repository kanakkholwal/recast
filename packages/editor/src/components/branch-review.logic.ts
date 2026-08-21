/**
 * Pure presentation logic for the branch review panel: how a dotted path and a
 * pair of JSON values become a row a human can read. The store-coupled parts
 * (loading, applying, discarding) stay in `branch-store.svelte.ts`.
 */

import { changeGroup, describeChange, type FieldChange } from "../lib/agent/branches";

/** How many values a row shows before it collapses to a summary. */
const INLINE_ARRAY_LIMIT = 3;
const INLINE_TEXT_LIMIT = 48;

/** Friendly names for the render-state collections a change can land in. */
const GROUP_LABELS: Record<string, string> = {
	annotations: "Annotations",
	audioSettings: "Audio",
	backgroundSettings: "Background",
	captionSettings: "Captions",
	cameraOverlay: "Camera",
	cursorSettings: "Cursor",
	cuts: "Cuts",
	musicClips: "Music",
	sceneAnimations: "Animations",
	segmentSpeeds: "Speed",
	splitPoints: "Split points",
	zoomRegions: "Zoom",
};

export function groupLabel(group: string): string {
	return GROUP_LABELS[group] ?? humanise(group);
}

/** `audioSettings.volume` -> `Volume`; `cuts.0.end` -> `Cut 1 end`. */
export function fieldLabel(field: string): string {
	const parts = field.split(".");
	if (parts.length === 1) return humanise(parts[0]);

	const [head, ...rest] = parts;
	const index = Number(rest[0]);
	if (Number.isInteger(index)) {
		const leaf = rest.slice(1);
		const row = `${singular(groupLabel(head))} ${index + 1}`;
		return leaf.length ? `${row} ${humanise(leaf.join(" ")).toLowerCase()}` : row;
	}
	return humanise(rest.join(" "));
}

/** Compact one side of a change for display in a fixed-width row. */
export function formatValue(value: unknown): string {
	if (value === null || value === undefined) return "—";
	if (typeof value === "boolean") return value ? "on" : "off";
	if (typeof value === "number") return formatNumber(value);
	if (typeof value === "string") return truncate(value, INLINE_TEXT_LIMIT);
	if (Array.isArray(value)) {
		if (value.length === 0) return "empty";
		if (value.length > INLINE_ARRAY_LIMIT) return `${value.length} items`;
		return value.map(formatValue).join(", ");
	}
	if (typeof value === "object") {
		const entries = Object.entries(value as Record<string, unknown>);
		if (entries.length === 0) return "empty";
		if (entries.length > INLINE_ARRAY_LIMIT) return `${entries.length} fields`;
		return entries
			.map(([key, inner]) => `${humanise(key).toLowerCase()} ${formatValue(inner)}`)
			.join(", ");
	}
	return String(value);
}

export interface ChangeRow {
	field: string;
	label: string;
	kind: ReturnType<typeof describeChange>;
	before: string;
	after: string;
}

export function toRow(change: FieldChange): ChangeRow {
	return {
		field: change.field,
		label: fieldLabel(change.field),
		kind: describeChange(change),
		before: formatValue(change.before),
		after: formatValue(change.after),
	};
}

export interface ChangeSection {
	group: string;
	label: string;
	rows: ChangeRow[];
}

/** Sections in first-appearance order, so the list reads like the state. */
export function toSections(changes: readonly FieldChange[]): ChangeSection[] {
	const sections = new Map<string, ChangeSection>();
	for (const change of changes) {
		const group = changeGroup(change.field);
		let section = sections.get(group);
		if (!section) {
			section = { group, label: groupLabel(group), rows: [] };
			sections.set(group, section);
		}
		section.rows.push(toRow(change));
	}
	return [...sections.values()];
}

/** One-line summary of a branch for the list row. */
export function summariseChanges(changes: readonly FieldChange[]): string {
	if (changes.length === 0) return "No changes";
	const groups = new Set(changes.map((change) => changeGroup(change.field)));
	const noun = changes.length === 1 ? "change" : "changes";
	const labels = [...groups].map(groupLabel);
	const where =
		labels.length <= 2
			? labels.join(" and ")
			: `${labels.slice(0, 2).join(", ")} +${labels.length - 2}`;
	return `${changes.length} ${noun} in ${where}`;
}

/** Relative age, coarse on purpose: an exact clock adds nothing here. */
export function relativeAge(atMs: number, nowMs: number): string {
	const seconds = Math.max(0, Math.round((nowMs - atMs) / 1000));
	if (seconds < 60) return "just now";
	const minutes = Math.round(seconds / 60);
	if (minutes < 60) return `${minutes}m ago`;
	const hours = Math.round(minutes / 60);
	if (hours < 24) return `${hours}h ago`;
	return `${Math.round(hours / 24)}d ago`;
}

function formatNumber(value: number): string {
	if (!Number.isFinite(value)) return String(value);
	if (Number.isInteger(value)) return String(value);
	return value.toFixed(2).replace(/\.?0+$/, "");
}

function truncate(text: string, limit: number): string {
	return text.length <= limit ? text : `${text.slice(0, limit - 1)}…`;
}

/** `audioSettings` / `trim_start` / `cuts.0` -> `Audio settings`. */
function humanise(key: string): string {
	const spaced = key
		.replace(/[._]+/g, " ")
		.replace(/([a-z0-9])([A-Z])/g, "$1 $2")
		.trim();
	if (!spaced) return key;
	return spaced.charAt(0).toUpperCase() + spaced.slice(1).toLowerCase();
}

function singular(label: string): string {
	if (label.endsWith("ies")) return `${label.slice(0, -3)}y`;
	return label.endsWith("s") ? label.slice(0, -1) : label;
}
