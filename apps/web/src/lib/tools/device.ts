/**
 * Per-device input-size budget for the in-browser tools.
 *
 * The real limit on these tools is memory, and it's device-dependent: a phone
 * has far less headroom than a desktop. Two things actually constrain us:
 *   1. The input file we read into memory (bounded by RAM).
 *   2. Decoded frames held during a pixel op (bounded by streaming one frame at
 *      a time in the worker, NOT by the file's length).
 * This module handles (1): estimate a safe input cap from the device, and check
 * a chosen file against it BEFORE we read it. An over-budget file isn't a dead
 * end, it's a funnel to the desktop app, which has no such limit.
 *
 * `computeMaxInputBytes` is pure (no `navigator`) so the policy is testable; the
 * rest reads the device and is SSR-safe.
 */

const MB = 1024 * 1024;

/**
 * The input-size cap (bytes) for a device with `memGB` GB of reported memory
 * (null if unknown) on `mobile`. Mobile is capped low regardless of report;
 * desktop scales with RAM inside a sane window.
 */
export function computeMaxInputBytes(memGB: number | null, mobile: boolean): number {
	if (mobile) {
		return (memGB != null && memGB <= 2 ? 120 : 200) * MB;
	}
	if (memGB == null) return 500 * MB; // unknown desktop: conservative middle
	const scaled = memGB * 128 * MB; // ~128 MB of input per reported GB
	return Math.max(256 * MB, Math.min(scaled, 1024 * MB));
}

/** `navigator.deviceMemory` in GB (Chromium only; null elsewhere). */
export function deviceMemoryGB(): number | null {
	if (typeof navigator === 'undefined') return null;
	const dm = (navigator as Navigator & { deviceMemory?: number }).deviceMemory;
	return typeof dm === 'number' ? dm : null;
}

/** Best-effort mobile detection: the UA-Client-Hints flag, else a UA regex. */
export function isMobile(): boolean {
	if (typeof navigator === 'undefined') return false;
	const uaData = (navigator as Navigator & { userAgentData?: { mobile?: boolean } }).userAgentData;
	if (uaData && typeof uaData.mobile === 'boolean') return uaData.mobile;
	return /Mobi|Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
}

export interface SizeBudget {
	/** Hard cap on input bytes for this device. */
	maxInputBytes: number;
	/** Human label for the UI, e.g. "~500 MB". */
	label: string;
}

/** The input budget for the current device. */
export function inputBudget(): SizeBudget {
	const bytes = computeMaxInputBytes(deviceMemoryGB(), isMobile());
	return { maxInputBytes: bytes, label: formatBytes(bytes) };
}

export interface SizeCheck {
	ok: boolean;
	/** Set when over budget — the message to show (with the desktop funnel). */
	reason?: string;
}

/** Whether `fileBytes` fits the budget; if not, a message that points at the app. */
export function checkFileSize(fileBytes: number, budget: SizeBudget): SizeCheck {
	if (fileBytes <= budget.maxInputBytes) return { ok: true };
	return {
		ok: false,
		reason: `This file is ${formatBytes(fileBytes)}. The in-browser tool handles up to about ${budget.label} on this device. For larger files, the Recast desktop app has no size limit.`,
	};
}

/** Compact size label: "~500 MB", "1.2 GB", "850 KB". */
export function formatBytes(bytes: number): string {
	if (bytes >= 1024 * MB) return `${(bytes / (1024 * MB)).toFixed(1)} GB`;
	if (bytes >= MB) return `~${Math.round(bytes / MB)} MB`;
	if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
	return `${bytes} B`;
}
