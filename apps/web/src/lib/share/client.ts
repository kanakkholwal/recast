/**
 * Browser-side helpers for the share page's engagement layer (comments +
 * reactions). Identity is name-only: an anonymous `sessionId` persisted per
 * browser plus a self-supplied display name. None of this requires an
 * account — the share page's primary audience are recipients who've never
 * heard of Recast.
 */

import { safeStorage } from "@recast/ui/persisted-state";

const SID_KEY = "recast.share.sid";
const NAME_KEY = "recast.share.name";

// Reaction set moved to `$lib/share/reactions` (id-based, server-safe).

export type ShareComment = {
	id: string;
	authorName: string;
	atSeconds: number;
	body: string;
	createdAt: number;
	mine: boolean;
};

export type ReactionCount = { emoji: string; count: number };

export type Engagement = {
	ok: boolean;
	commentsEnabled: boolean;
	canManage: boolean;
	comments: ShareComment[];
	reactions: ReactionCount[];
	myReactions: string[];
};

/** Stable anonymous fingerprint for this browser. Created lazily. Returns ""
 *  during SSR (no window) so the server render stays identity-free. */
export function shareSessionId(): string {
	if (typeof window === "undefined") return "";
	let sid = safeStorage.get<string>(SID_KEY, "");
	if (!sid) {
		sid = crypto.randomUUID();
		safeStorage.set(SID_KEY, sid);
	}
	return sid;
}

export function storedViewerName(): string {
	return safeStorage.get<string>(NAME_KEY, "");
}

export function rememberViewerName(name: string): void {
	const trimmed = name.trim();
	if (trimmed) safeStorage.set(NAME_KEY, trimmed);
}

export async function loadEngagement(slug: string, sessionId: string): Promise<Engagement> {
	const res = await fetch(
		`/api/share/${slug}/comments?sessionId=${encodeURIComponent(sessionId)}`,
	);
	// Throw on non-ok so callers can distinguish a genuine failure (network /
	// 5xx / gate) from an empty-but-loaded thread. The page surfaces the former
	// as a retry affordance instead of a false "No comments yet".
	if (!res.ok) throw new Error(`Couldn't load engagement (${res.status})`);
	return (await res.json()) as Engagement;
}

export async function postComment(
	slug: string,
	input: { sessionId: string; authorName: string; atSeconds: number; body: string },
): Promise<ShareComment> {
	const res = await fetch(`/api/share/${slug}/comments`, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(input),
	});
	if (!res.ok) {
		const message = await res.text().catch(() => "");
		throw new Error(message || "Couldn't post comment");
	}
	const data = (await res.json()) as { comment: ShareComment };
	return data.comment;
}

export async function deleteComment(
	slug: string,
	commentId: string,
	sessionId: string,
): Promise<void> {
	const res = await fetch(
		`/api/share/${slug}/comments/${commentId}?sessionId=${encodeURIComponent(sessionId)}`,
		{ method: "DELETE" },
	);
	if (!res.ok) {
		const message = await res.text().catch(() => "");
		throw new Error(message || "Couldn't delete comment");
	}
}

export async function toggleReaction(
	slug: string,
	input: { sessionId: string; emoji: string; atSeconds: number },
): Promise<boolean> {
	const res = await fetch(`/api/share/${slug}/reactions`, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(input),
	});
	if (!res.ok) {
		const message = await res.text().catch(() => "");
		throw new Error(message || "Couldn't react");
	}
	const data = (await res.json()) as { added: boolean };
	return data.added;
}
