/**
 * Pure engagement helpers for the share page. Most of the share page's
 * engagement code is irreducibly coupled to the player API, the DOM, and shared
 * page-reactive state, so it stays in the component; this is the one piece with
 * real, bug-prone logic worth isolating — the optimistic reaction toggle.
 */

import type { ReactionCount } from "./client";

export interface ReactionState {
	/** Emojis the current viewer has reacted with. */
	myReactions: Set<string>;
	/** Aggregate counts per emoji. */
	reactions: ReactionCount[];
}

/**
 * Optimistic local update for setting the viewer's reaction to `emoji`. A
 * viewer holds a SINGLE reaction, so this mirrors the server:
 *   - tapping the current reaction removes it (toggle off)
 *   - tapping a different one switches in place (drops the previous, adds this)
 * Counts are bumped/dropped accordingly (entries removed at zero). Returns fresh
 * objects; does not mutate the input.
 */
export function toggleReactionState(
	current: ReactionState,
	emoji: string,
): ReactionState {
	const next = current.reactions.map((r) => ({ ...r }));
	const dec = (e: string) => {
		const i = next.findIndex((r) => r.emoji === e);
		if (i >= 0) {
			next[i].count -= 1;
			if (next[i].count <= 0) next.splice(i, 1);
		}
	};
	const inc = (e: string) => {
		const i = next.findIndex((r) => r.emoji === e);
		if (i >= 0) next[i].count += 1;
		else next.push({ emoji: e, count: 1 });
	};

	if (current.myReactions.has(emoji)) {
		dec(emoji);
		return { myReactions: new Set(), reactions: next };
	}
	// Switching reactions: drop whatever they had, then add the new one.
	for (const prev of current.myReactions) dec(prev);
	inc(emoji);
	return { myReactions: new Set([emoji]), reactions: next };
}
