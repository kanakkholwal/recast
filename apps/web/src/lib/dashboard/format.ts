/** Display formatters for the dashboard — pure, no side effects. */

/** `252` → `"4:12"`, `3870` → `"1:04:30"`. */
export function formatDuration(totalSec: number): string {
	const s = Math.max(0, Math.round(totalSec));
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	const sec = s % 60;
	const pad = (n: number) => String(n).padStart(2, "0");
	return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

/** `191000000` → `"182 MB"`. */
export function formatBytes(bytes: number): string {
	if (bytes <= 0) return "0 MB";
	const units = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.min(
		units.length - 1,
		Math.floor(Math.log(bytes) / Math.log(1024)),
	);
	const val = bytes / 1024 ** i;
	const rounded = val >= 100 || i <= 1 ? Math.round(val) : Math.round(val * 10) / 10;
	return `${rounded} ${units[i]}`;
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

/** `1024` → `"1,024"`. */
export function formatCount(n: number): string {
	return n.toLocaleString("en-US");
}
