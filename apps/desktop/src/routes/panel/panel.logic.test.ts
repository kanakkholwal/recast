import { describe, expect, it } from "vitest";
import type { CaptureIntentState } from "@recast/editor/lib/wire-types";
import {
	canonicalIntent,
	clampFpsToDisplay,
	formatRecordingTimer,
	intentToTargetType,
	lastSourceToTarget,
	targetToLastSource,
	targetTypeToIntent,
	type TargetSource,
} from "./panel.logic";

describe("panel <-> capture-intent target mapping", () => {
	it("maps the panel source type to the intent target type", () => {
		expect(targetTypeToIntent("monitor")).toBe("display");
		expect(targetTypeToIntent("window")).toBe("window");
		expect(targetTypeToIntent("region")).toBe("region");
	});

	it("maps the intent target type back to the panel source type", () => {
		expect(intentToTargetType("display")).toBe("monitor");
		expect(intentToTargetType("window")).toBe("window");
		expect(intentToTargetType("region")).toBe("region");
		expect(intentToTargetType(null)).toBeNull();
		expect(intentToTargetType(undefined)).toBeNull();
		expect(intentToTargetType("bogus")).toBeNull();
	});

	it("round-trips every source type", () => {
		for (const t of ["monitor", "window", "region"] as const) {
			expect(intentToTargetType(targetTypeToIntent(t))).toBe(t);
		}
	});
});

describe("last-source round trip", () => {
	it("preserves a monitor source", () => {
		const src: TargetSource = { type: "monitor", id: 5, label: "Primary Display" };
		expect(lastSourceToTarget(targetToLastSource(src))).toMatchObject({
			type: "monitor",
			id: 5,
			label: "Primary Display",
		});
	});

	it("preserves a region source with its rect", () => {
		const src: TargetSource = {
			type: "region",
			id: 0,
			label: "Region",
			region: { x: 1, y: 2, width: 3, height: 4 },
		};
		const round = lastSourceToTarget(targetToLastSource(src));
		expect(round.type).toBe("region");
		expect(round.region).toEqual({ x: 1, y: 2, width: 3, height: 4 });
	});
});

describe("fps clamping to display", () => {
	it("caps a desired fps to the monitor refresh", () => {
		const monitor: TargetSource = { type: "monitor", id: 1, label: "d", refreshHz: 60 };
		expect(clampFpsToDisplay(144, monitor)).toBe(60);
		expect(clampFpsToDisplay(30, monitor)).toBe(30);
	});

	it("passes through for Auto / non-monitor / unknown refresh", () => {
		expect(
			clampFpsToDisplay(null, { type: "monitor", id: 1, label: "d", refreshHz: 60 }),
		).toBeNull();
		expect(clampFpsToDisplay(120, { type: "window", id: 1, label: "w" })).toBe(120);
		expect(clampFpsToDisplay(120, null)).toBe(120);
	});
});

describe("canonicalIntent (echo-guard against the freeze loop)", () => {
	it("treats explicit TS nulls the same as the backend's omitted fields", () => {
		// What the panel sends: explicit nulls for empty optionals.
		const sent: CaptureIntentState = {
			targetType: "display",
			targetId: 1,
			region: null,
			options: {
				systemAudio: true,
				microphone: false,
				microphoneDeviceId: null,
				camera: false,
				cameraDeviceId: null,
			},
			countdown: null,
			activeProfileId: null,
		};
		// What the backend echoes back (skip_serializing_if omits the Nones).
		const echoed: CaptureIntentState = {
			targetType: "display",
			targetId: 1,
			options: { systemAudio: true, microphone: false, camera: false },
		};
		// These MUST compare equal, or the push effect + listener loop forever.
		expect(canonicalIntent(sent)).toBe(canonicalIntent(echoed));
	});

	it("is independent of key order", () => {
		const a = { targetId: 2, targetType: "window", options: { camera: true, systemAudio: false } };
		const b = { options: { systemAudio: false, camera: true }, targetType: "window", targetId: 2 };
		expect(canonicalIntent(a as CaptureIntentState)).toBe(canonicalIntent(b as CaptureIntentState));
	});

	it("still distinguishes genuinely different intents", () => {
		const base: CaptureIntentState = {
			targetType: "display",
			targetId: 1,
			options: { systemAudio: true },
		};
		const otherSource: CaptureIntentState = {
			targetType: "window",
			targetId: 1,
			options: { systemAudio: true },
		};
		const otherDevice: CaptureIntentState = {
			targetType: "display",
			targetId: 1,
			options: { systemAudio: true, microphone: true, microphoneDeviceId: "mic-1" },
		};
		expect(canonicalIntent(base)).not.toBe(canonicalIntent(otherSource));
		expect(canonicalIntent(base)).not.toBe(canonicalIntent(otherDevice));
	});

	it("null and empty compare equal to itself (idempotent)", () => {
		expect(canonicalIntent(null)).toBe("");
		const i: CaptureIntentState = { targetId: 0, options: { systemAudio: true } };
		expect(canonicalIntent(i)).toBe(canonicalIntent({ ...i }));
	});
});

describe("recording timer formatting", () => {
	it("formats mm:ss", () => {
		expect(formatRecordingTimer(0)).toBe("00:00");
		expect(formatRecordingTimer(5)).toBe("00:05");
		expect(formatRecordingTimer(65)).toBe("01:05");
		expect(formatRecordingTimer(3599)).toBe("59:59");
	});
});
