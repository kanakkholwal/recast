/**
 * Pure filtering/ranking/highlighting for the command palette. The component
 * keeps run/close/keyboard-nav; these are the framework-free bits.
 */

import type { PaletteCommand } from "$lib/stores/command-palette.svelte";

/** Score-based relevance: title > description > keywords > category. Empty query = all. */
export function matchScore(cmd: PaletteCommand, q: string): number {
	if (!q) return 1;
	const needle = q.toLowerCase();
	const t = cmd.title.toLowerCase();
	if (t.startsWith(needle)) return 100;
	if (t.includes(needle)) return 80;
	if ((cmd.description ?? "").toLowerCase().includes(needle)) return 60;
	if ((cmd.keywords ?? []).some((k) => k.toLowerCase().includes(needle)))
		return 40;
	if (cmd.category.toLowerCase().includes(needle)) return 20;
	return 0;
}

/** Commands with a non-zero score, ranked most-relevant first. */
export function rankCommands(
	commands: PaletteCommand[],
	query: string,
): PaletteCommand[] {
	return commands
		.map((c) => ({ cmd: c, score: matchScore(c, query) }))
		.filter((x) => x.score > 0)
		.sort((a, b) => b.score - a.score)
		.map((x) => x.cmd);
}

/**
 * When the query is empty, group by category. Otherwise show a flat, relevance-
 * ranked list under a single "Results" heading, which matches the search-results
 * mental model.
 */
export function groupCommands(
	filtered: PaletteCommand[],
	query: string,
): [string, PaletteCommand[]][] {
	if (query.trim()) return [["Results", filtered]];
	const map = new Map<string, PaletteCommand[]>();
	for (const cmd of filtered) {
		if (!map.has(cmd.category)) map.set(cmd.category, []);
		map.get(cmd.category)!.push(cmd);
	}
	return Array.from(map.entries());
}

/** Split `text` into runs, flagging the parts that match `search` for emphasis. */
export function highlight(
	text: string,
	search: string,
): { text: string; hl: boolean }[] {
	if (!search.trim()) return [{ text, hl: false }];
	const escaped = search.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	const regex = new RegExp(`(${escaped})`, "gi");
	return text
		.split(regex)
		.filter((p) => p.length > 0)
		.map((part) => ({
			text: part,
			hl: part.toLowerCase() === search.toLowerCase(),
		}));
}
