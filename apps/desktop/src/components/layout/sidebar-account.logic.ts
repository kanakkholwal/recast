export function monogram(name: string): string {
	const parts = name.trim().split(/\s+/).filter(Boolean);
	if (parts.length === 0) return "?";
	if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
	return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

export interface SharesMeter {
	label: string;
	pct: number;
}

/** A usable shares gauge only when the plan defines a finite limit. */
export function sharesMeter(used: number, limit: number | null | undefined): SharesMeter | null {
	if (!limit || limit <= 0) return null;
	const pct = Math.max(0, Math.min(100, Math.round((used / limit) * 100)));
	return { label: `${used} of ${limit} shares`, pct };
}
