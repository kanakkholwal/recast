/** Display formatters for the dashboard — pure, no side effects. */

import { formatBytes as formatBytesBase } from "@recast/editor/lib/format/bytes";

/** `252` → `"4:12"`, `3870` → `"1:04:30"`. */
export function formatDuration(totalSec: number): string {
	const s = Math.max(0, Math.round(totalSec));
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	const sec = s % 60;
	const pad = (n: number) => String(n).padStart(2, "0");
	return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

/** `191000000` → `"182 MB"`. Storage cells read better empty as "0 MB" than "0 B". */
export function formatBytes(bytes: number): string {
	return formatBytesBase(bytes, { zeroLabel: "0 MB" });
}

/** `1747000000000` → `"May 17, 2026"`. */
export function formatDate(ts: number): string {
	return new Date(ts).toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
		year: "numeric",
	});
}

/** Human "time ago" for recent items, absolute date beyond a month. */
export function formatRelative(ts: number): string {
	const diff = Date.now() - ts;
	const day = 86_400_000;
	if (diff < 0) return "Just now";
	if (diff < day) return "Today";
	if (diff < 2 * day) return "Yesterday";
	if (diff < 7 * day) return `${Math.floor(diff / day)} days ago`;
	if (diff < 30 * day) return `${Math.floor(diff / (7 * day))} wk ago`;
	return formatDate(ts);
}

/**
 * Share-link expiry label. `formatRelative` only speaks the past ("Today", "2
 * days ago") and collapses any future date to "Just now", so expiries need
 * their own future-aware formatter: "Expires in 7 days" while live, "Expired"
 * once the deadline passes (paired with the `expired` flag for styling).
 */
export function formatExpiry(expiresAt: number): { expired: boolean; label: string } {
	const diff = expiresAt - Date.now();
	if (diff <= 0) return { expired: true, label: "Expired" };
	const min = 60_000;
	const hour = 3_600_000;
	const day = 86_400_000;
	let rel: string;
	if (diff < hour) rel = `${Math.max(1, Math.round(diff / min))} min`;
	else if (diff < day) {
		const h = Math.round(diff / hour);
		rel = `${h} hr`;
	} else {
		const d = Math.round(diff / day);
		rel = `${d} day${d === 1 ? "" : "s"}`;
	}
	return { expired: false, label: `Expires in ${rel}` };
}

/** `1024` → `"1,024"`. */
export function formatCount(n: number): string {
	return n.toLocaleString("en-US");
}

/** `0.00098` → `"<1%"`. Raw ratios printed at float precision are noise. */
export function formatPct(value: number | null | undefined): string {
	const pct = Math.min(100, Math.max(0, value ?? 0));
	if (pct === 0) return "0%";
	return pct < 1 ? "<1%" : `${Math.round(pct)}%`;
}

/** Bar width for a 0-100 value. Non-zero usage keeps a visible sliver. */
export function barWidth(value: number | null | undefined): number {
	const pct = Math.min(100, Math.max(0, value ?? 0));
	return pct > 0 && pct < 2 ? 2 : Math.round(pct);
}
