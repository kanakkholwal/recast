/**
 * PresetPicker helpers: fuzzy search scoring, thumbnail styling, aspect/wallpaper
 * mapping, and the keyboard-grid navigation model. Functions take a structural
 * `PresetLike` so this module doesn't depend on the component's `Preset` type.
 */

export interface PresetLike {
	label: string;
	category: string;
	aspect: string;
	description?: string;
	keywords?: string[];
	bg?: string;
	value?: string;
}

/**
 * Relevance of a preset to query `q` (higher = better, 0 = no match). Ranks
 * label/category prefix highest, then substring matches, then description,
 * keywords, and aspect. Empty query matches everything (returns 1).
 */
export function score(p: PresetLike, q: string): number {
	if (!q) return 1;
	const n = q.toLowerCase();
	const label = p.label.toLowerCase();
	const cat = p.category.toLowerCase();
	if (label.startsWith(n)) return 100;
	if (cat.startsWith(n)) return 90;
	if (label.includes(n)) return 80;
	if (cat.includes(n)) return 60;
	if ((p.description ?? "").toLowerCase().includes(n)) return 40;
	if ((p.keywords ?? []).some((k) => k.toLowerCase().includes(n))) return 30;
	if (p.aspect.toLowerCase().includes(n)) return 20;
	return 0;
}

/** Inline style for a preset's thumbnail background. */
export function bgPreviewStyle(p: PresetLike): string {
	if ((p.bg === "gradient" || p.bg === "color") && p.value) return `background:${p.value}`;
	return "background:var(--color-muted)";
}

/**
 * WYSIWYG frame inset percent. `padding` is a percent of the shorter source edge
 * applied each side, so the video occupies `1/(1+2p)` of that edge, mirrored
 * here so the thumbnail frames like the real export. Capped at 20%.
 */
export function frameInsetPct(padding: number): number {
	const p = Math.max(0, padding) / 100;
	return Math.min(20, (p / (1 + 2 * p)) * 100);
}

/** The registry id for a wallpaper preset (strips the `asset:`/`asset://`). */
export function wallpaperId(p: PresetLike): string {
	return (p.value ?? "").replace(/^asset:\/\/?/, "").replace(/^asset:/, "");
}

/** Tailwind aspect class for a preset aspect string. */
export function aspectClass(aspect: string): string {
	switch (aspect) {
		case "1:1":
			return "aspect-square";
		case "9:16":
			return "aspect-[9/16]";
		case "16:9":
			return "aspect-video";
		case "1.91:1":
			return "aspect-[1.91/1]";
		default:
			return "aspect-video";
	}
}

// --- Keyboard-grid navigation: builds the per-category layout and cursor moves; the component keeps state, refs and effects.

export interface Cell<T> {
	preset: T;
	index: number;
}

export interface PresetModel<T> {
	groups: { category: string; rows: Cell<T>[][] }[];
	flat: T[];
	rows: Cell<T>[][];
}

/** Presets matching `query`, best-first; all presets (in input order) if empty. */
export function filterPresets<T extends PresetLike>(presets: T[], query: string): T[] {
	return presets
		.map((p) => ({ p, s: score(p, query) }))
		.filter((x) => x.s > 0)
		.sort((a, b) => b.s - a.s)
		.map((x) => x.p);
}

/**
 * Group filtered presets by category (a single "Results" group while searching),
 * pinning the currently-applied preset to a "Current" group on top.
 */
export function groupPresets<T extends PresetLike>(
	filtered: T[],
	query: string,
	current: T | null,
): [string, T[]][] {
	if (query.trim()) return [["Results", filtered]];
	const map = new Map<string, T[]>();
	for (const p of filtered) {
		// The pinned copy is the only one: listing it twice made the cursor pass the same preset at two indices.
		if (p === current) continue;
		let bucket = map.get(p.category);
		if (!bucket) {
			bucket = [];
			map.set(p.category, bucket);
		}
		bucket.push(p);
	}
	const entries = [...map.entries()].filter(([, items]) => items.length > 0);
	if (current) entries.unshift(["Current", [current]]);
	return entries;
}

/** Esc clears a typed query first and only closes once the field is empty. */
export function resolveEscape(query: string): "clear" | "close" {
	return query.trim() ? "clear" : "close";
}

/** Stable option id for `aria-activedescendant` on the listbox cursor. */
export function optionId(baseId: string, index: number): string {
	return `${baseId}-option-${index}`;
}

/**
 * Chunk grouped presets into rows of `cols`, assigning each cell a unique running
 * `index` so navigation never relies on `indexOf`.
 */
export function buildModel<T>(grouped: [string, T[]][], cols: number): PresetModel<T> {
	const groups: PresetModel<T>["groups"] = [];
	const flat: T[] = [];
	const rows: Cell<T>[][] = [];
	let counter = 0;
	for (const [category, items] of grouped) {
		const groupRows: Cell<T>[][] = [];
		for (let i = 0; i < items.length; i += cols) {
			const cells: Cell<T>[] = [];
			for (let j = i; j < Math.min(i + cols, items.length); j++) {
				const cell = { preset: items[j], index: counter++ };
				cells.push(cell);
				flat.push(items[j]);
			}
			groupRows.push(cells);
			rows.push(cells);
		}
		groups.push({ category, rows: groupRows });
	}
	return { groups, flat, rows };
}

/** [rowPos, col] of `index` within the global row list; [0,0] if not found. */
export function locateCell<T>(rows: Cell<T>[][], index: number): [number, number] {
	for (let r = 0; r < rows.length; r++) {
		const c = rows[r].findIndex((cell) => cell.index === index);
		if (c !== -1) return [r, c];
	}
	return [0, 0];
}

/**
 * New cursor index after a vertical move (jump a whole row, preserving the
 * column, clamped when the target row is shorter), or null if there's no row in
 * that direction.
 */
export function rowMoveIndex<T>(model: PresetModel<T>, index: number, dir: 1 | -1): number | null {
	const [row, col] = locateCell(model.rows, index);
	const target = model.rows[row + dir];
	if (!target) return null;
	return target[Math.min(col, target.length - 1)].index;
}

/** Clamp an index into [0, len-1] (len 0 → 0). */
export function clampIndex(index: number, len: number): number {
	return Math.max(0, Math.min(len - 1, index));
}
