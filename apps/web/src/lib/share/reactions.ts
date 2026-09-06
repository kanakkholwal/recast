/**
 * Reaction registry — the single source of truth for the share page's
 * sentiment reactions. Each reaction has a STABLE `id` that is what we persist
 * (`share_reaction.emoji` holds the id, not a raw glyph) so the rendered icon
 * can be swapped later without a data migration. `emoji` is kept only as a
 * text/analytics fallback; the viewer-facing icon is a @recast/icons component mapped
 * by `id` on the client (design system is @recast/icons-only).
 *
 * Pure + component-free so both server (rate-limit allow-list, owner analytics)
 * and client can import it.
 */

export type ReactionId = "like" | "love" | "laugh" | "wow" | "celebrate" | "fire";

export type ReactionDef = {
	id: ReactionId;
	/** Short verb shown on hover / as the accessible label. */
	label: string;
	/** Text fallback for non-icon contexts (analytics exports, emails). */
	emoji: string;
	/** Accent hue (HSL degrees) for the active/tinted state. */
	hue: number;
};

export const REACTIONS: readonly ReactionDef[] = [
	{ id: "like", label: "Like", emoji: "👍", hue: 217 },
	{ id: "love", label: "Love", emoji: "❤️", hue: 347 },
	{ id: "laugh", label: "Haha", emoji: "😂", hue: 45 },
	{ id: "wow", label: "Wow", emoji: "😮", hue: 275 },
	{ id: "celebrate", label: "Celebrate", emoji: "🎉", hue: 152 },
	{ id: "fire", label: "Fire", emoji: "🔥", hue: 18 },
] as const;

/** Allow-list of persisted ids — used to bound reaction writes server-side. */
export const REACTION_IDS: readonly string[] = REACTIONS.map((r) => r.id);

const BY_ID = new Map<string, ReactionDef>(REACTIONS.map((r) => [r.id, r]));

export function reactionById(id: string): ReactionDef | undefined {
	return BY_ID.get(id);
}

/** Display glyph for a stored reaction id, falling back to the raw value for
 *  legacy rows that stored a bare emoji. */
export function reactionGlyph(id: string): string {
	return BY_ID.get(id)?.emoji ?? id;
}
