/** Pure mappers + fps/timer math for the recording panel window. */

import type { DeviceResolution } from "@recast/editor/lib/profiles";
import type { CaptureIntentState, LastSource } from "$lib/recorder-types";

export type TargetSource = {
	type: "monitor" | "window" | "region";
	id: number;
	label: string;
	/** Monitor refresh rate in Hz (monitors only); caps the useful capture
	 *  fps so we never record above what the display can present. */
	refreshHz?: number;
	region?: {
		x: number;
		y: number;
		width: number;
		height: number;
	};
};

/**
 * Cap a desired capture fps to a monitor source's refresh rate. `null` (Auto)
 * and non-monitor / unknown-refresh sources pass through unchanged. The
 * backend still clamps to its 24–240 range.
 */
export function clampFpsToDisplay(
	desired: number | null,
	source: TargetSource | null,
): number | null {
	if (desired == null) return null;
	const cap = source?.type === "monitor" ? source.refreshHz : undefined;
	return cap && cap >= 1 ? Math.min(desired, cap) : desired;
}

/** Persisted `LastSource` → the panel's in-memory selected source. */
export function lastSourceToTarget(last: LastSource): TargetSource {
	return {
		type: last.kind === "window" ? "window" : last.kind === "region" ? "region" : "monitor",
		id: last.id,
		label: last.label,
		region:
			last.kind === "region" && last.regionWidth != null && last.regionHeight != null
				? {
						x: last.regionX ?? 0,
						y: last.regionY ?? 0,
						width: last.regionWidth,
						height: last.regionHeight,
					}
				: undefined,
	};
}

/** Selected source → the `LastSource` payload persisted for next launch. */
export function targetToLastSource(source: TargetSource): LastSource {
	return {
		kind: source.type === "monitor" ? "monitor" : source.type === "window" ? "window" : "region",
		id: source.id,
		label: source.label,
		regionX: source.region?.x ?? null,
		regionY: source.region?.y ?? null,
		regionWidth: source.region?.width ?? null,
		regionHeight: source.region?.height ?? null,
	};
}

/** Panel source type → the backend `CaptureIntent.targetType` ("monitor" is
 *  the panel's word for a full display). */
export function targetTypeToIntent(type: TargetSource["type"]): string {
	return type === "monitor" ? "display" : type;
}

/** `CaptureIntent.targetType` → the panel's source type, or null if unset. */
export function intentToTargetType(t: string | null | undefined): TargetSource["type"] | null {
	if (t === "display") return "monitor";
	if (t === "window" || t === "region") return t;
	return null;
}

/**
 * Canonical string form of a capture intent, for the panel's echo guard. The
 * backend serializes `CaptureIntent`/`RecordingOptions` with
 * `skip_serializing_if = "Option::is_none"`, so its `capture-intent:changed`
 * echo OMITS null fields (region, device ids, countdown, activeProfileId) that
 * the panel writes explicitly. Raw `JSON.stringify` therefore never matched the
 * echo of what the panel just sent, spinning the push `$effect` and the change
 * listener into an infinite loop that froze the panel on open. Stripping nullish
 * fields and sorting keys makes the TS-null and Rust-omit shapes compare equal.
 */
export function canonicalIntent(intent: CaptureIntentState | null): string {
	if (intent == null) return "";
	return JSON.stringify(stripNullish(intent));
}

function stripNullish(value: unknown): unknown {
	if (Array.isArray(value)) return value.map(stripNullish);
	if (value !== null && typeof value === "object") {
		const out: Record<string, unknown> = {};
		for (const key of Object.keys(value as Record<string, unknown>).sort()) {
			const v = stripNullish((value as Record<string, unknown>)[key]);
			if (v !== null && v !== undefined) out[key] = v;
		}
		return out;
	}
	return value;
}

/** Elapsed seconds → `MM:SS` (minutes zero-padded; recordings stay short). */
export function formatRecordingTimer(elapsedSeconds: number): string {
	const s = Math.max(0, Math.floor(elapsedSeconds));
	const mm = Math.floor(s / 60)
		.toString()
		.padStart(2, "0");
	const ss = (s % 60).toString().padStart(2, "0");
	return `${mm}:${ss}`;
}

/** The panel's half of a capture intent: everything it actually drives. */
export interface PanelSelection {
	source: TargetSource | null;
	systemAudio: boolean;
	micOn: boolean;
	micDeviceId: string | null;
	cameraOn: boolean;
	/** Rust wants the DirectShow friendly name, not the browser device id. */
	cameraName: string | null;
}

/**
 * Fold the panel's selection into an intent, preserving the fields it does not
 * own (fps, quality, countdown, profile) from `base`.
 */
export function buildCaptureIntent(
	base: CaptureIntentState | null,
	selection: PanelSelection,
): CaptureIntentState {
	const previous: CaptureIntentState = base ?? { targetId: 0, options: { systemAudio: true } };
	const { source } = selection;
	return {
		...previous,
		targetType: source ? targetTypeToIntent(source.type) : null,
		targetId: source?.id ?? 0,
		region: source?.type === "region" && source.region ? source.region : null,
		options: {
			...previous.options,
			systemAudio: selection.systemAudio,
			microphone: selection.micOn,
			microphoneDeviceId: selection.micOn ? selection.micDeviceId : null,
			camera: selection.cameraOn,
			cameraDeviceId: selection.cameraOn ? selection.cameraName : null,
		},
	};
}

/** Placeholder label for a source named by an intent, before enrichment. */
export function intentSourceLabel(type: TargetSource["type"], targetId: number): string {
	if (type === "window") return `Window ${targetId}`;
	if (type === "region") return "Region";
	return `Display ${targetId}`;
}

/** The source an externally-set intent names, or `null` if it names none. */
export function sourceFromIntent(intent: CaptureIntentState): TargetSource | null {
	const type = intentToTargetType(intent.targetType);
	if (!type) return null;
	return {
		type,
		id: intent.targetId,
		label: intentSourceLabel(type, intent.targetId),
		region: type === "region" ? (intent.region ?? undefined) : undefined,
	};
}

/** What a resolved device means for the panel's toggle and its warning line. */
export interface DeviceOutcome<T> {
	/** Carried through so a caller can still tell "none requested" from
	 *  "requested but unavailable"; the two differ in teardown. */
	kind: DeviceResolution<T>["kind"];
	on: boolean;
	device: T | null;
	warning: string | null;
}

/**
 * Turn a [`DeviceResolution`] into the panel's on/off + warning.
 *
 * `describe` names the device for the fallback message; `noun` names the class
 * of device for the missing one.
 */
export function deviceOutcome<T>(
	resolution: DeviceResolution<T>,
	profileName: string,
	noun: string,
	describe: (device: T) => string,
): DeviceOutcome<T> {
	switch (resolution.kind) {
		case "matched":
			return { kind: "matched", on: true, device: resolution.device, warning: null };
		case "fallback":
			return {
				kind: "fallback",
				on: true,
				device: resolution.device,
				warning: `“${resolution.requestedLabel}” unavailable, using “${describe(resolution.device)}”`,
			};
		case "missing":
			return {
				kind: "missing",
				on: false,
				device: null,
				warning: `“${profileName}” wants a ${noun} but none is available`,
			};
		default:
			return { kind: "none", on: false, device: null, warning: null };
	}
}

/** One step of the mic meter: a sqrt curve lifts quiet speech, then fast-attack
 *  (jump up) / slow-release (ease down) makes it read like a real level meter. */
export function smoothMicLevel(prev: number, payload: number): number {
	const raw = Math.min(1, Math.sqrt(Math.max(0, payload)) * 1.7);
	return raw > prev ? raw : prev * 0.7 + raw * 0.3;
}
