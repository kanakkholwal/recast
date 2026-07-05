/** Pure progress/label helpers for corner-notifications. */

/** Clamped upload percentage (0–100); 0 when total is unknown/zero. */
export function uploadPct(bytesSent: number, totalBytes: number): number {
	if (!totalBytes) return 0;
	return Math.min(100, Math.round((bytesSent / totalBytes) * 100));
}

// Only the cloud-side phases surface here; export has its own progress UI.
export function cloudPhaseLabel(phase: string): string {
	switch (phase) {
		case "preparing":
			return "Preparing…";
		case "uploading":
			return "Uploading to Recast Cloud";
		case "finalizing":
			return "Finalizing…";
		case "sharing":
			return "Creating share link…";
		default:
			return "Sharing…";
	}
}
