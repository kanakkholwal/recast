import { describe, expect, it } from "vitest";
import type { CaptureIntentState } from "$lib/recorder-types";
import {
	buildCaptureIntent,
	canonicalIntent,
	clampFpsToDisplay,
	deviceOutcome,
	formatRecordingTimer,
	intentToTargetType,
	lastSourceToTarget,
	type PanelSelection,
	sourceFromIntent,
	type TargetSource,
	targetToLastSource,
	targetTypeToIntent,
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

describe("buildCaptureIntent", () => {
	const selection: PanelSelection = {
		source: { type: "monitor", id: 2, label: "Display 2" },
		systemAudio: true,
		micOn: false,
		micDeviceId: null,
		cameraOn: false,
		cameraName: null,
	};

	it("names the selected source", () => {
		const intent = buildCaptureIntent(null, selection);

		expect([intent.targetType, intent.targetId]).toEqual(["display", 2]);
	});

	it("reports no source when nothing is selected", () => {
		const intent = buildCaptureIntent(null, { ...selection, source: null });

		expect([intent.targetType, intent.targetId]).toEqual([null, 0]);
	});

	it("carries a region source's rectangle", () => {
		const region = { x: 1, y: 2, width: 3, height: 4 };

		const intent = buildCaptureIntent(null, {
			...selection,
			source: { type: "region", id: 0, label: "Region", region },
		});

		expect(intent.region).toEqual(region);
	});

	it("sends no region for a monitor source", () => {
		expect(buildCaptureIntent(null, selection).region).toBeNull();
	});

	/// The panel drives source + devices; everything else belongs to the CLI.
	it("preserves fields the panel does not own", () => {
		const base: CaptureIntentState = {
			targetId: 0,
			options: { systemAudio: true, fps: 60, quality: "high" },
			countdown: 5,
		};

		const intent = buildCaptureIntent(base, selection);

		expect([intent.countdown, intent.options.fps, intent.options.quality]).toEqual([5, 60, "high"]);
	});

	it("clears the mic id when the mic is off", () => {
		const intent = buildCaptureIntent(null, {
			...selection,
			micOn: false,
			micDeviceId: "mic-1",
		});

		expect(intent.options.microphoneDeviceId).toBeNull();
	});

	it("sends the mic id when the mic is on", () => {
		const intent = buildCaptureIntent(null, {
			...selection,
			micOn: true,
			micDeviceId: "mic-1",
		});

		expect(intent.options.microphoneDeviceId).toBe("mic-1");
	});

	it("clears the camera name when the camera is off", () => {
		const intent = buildCaptureIntent(null, {
			...selection,
			cameraOn: false,
			cameraName: "FaceTime HD",
		});

		expect(intent.options.cameraDeviceId).toBeNull();
	});

	it("sends the camera's friendly name, not a browser id", () => {
		const intent = buildCaptureIntent(null, {
			...selection,
			cameraOn: true,
			cameraName: "FaceTime HD",
		});

		expect(intent.options.cameraDeviceId).toBe("FaceTime HD");
	});
});

describe("sourceFromIntent", () => {
	function intent(over: Partial<CaptureIntentState>): CaptureIntentState {
		return { targetId: 0, options: { systemAudio: true }, ...over };
	}

	it("reads nothing from an intent with no target", () => {
		expect(sourceFromIntent(intent({ targetType: null }))).toBeNull();
	});

	it("labels a display by its id", () => {
		expect(sourceFromIntent(intent({ targetType: "display", targetId: 3 }))?.label).toBe(
			"Display 3",
		);
	});

	it("labels a window by its id", () => {
		expect(sourceFromIntent(intent({ targetType: "window", targetId: 7 }))?.label).toBe("Window 7");
	});

	it("labels a region without an id", () => {
		expect(sourceFromIntent(intent({ targetType: "region" }))?.label).toBe("Region");
	});

	it("carries a region's rectangle through", () => {
		const region = { x: 1, y: 2, width: 3, height: 4 };

		expect(sourceFromIntent(intent({ targetType: "region", region }))?.region).toEqual(region);
	});

	it("leaves a display's region unset", () => {
		expect(
			sourceFromIntent(
				intent({ targetType: "display", region: { x: 1, y: 2, width: 3, height: 4 } }),
			)?.region,
		).toBeUndefined();
	});

	it("round-trips through buildCaptureIntent", () => {
		const source = sourceFromIntent(intent({ targetType: "window", targetId: 9 }));

		const rebuilt = buildCaptureIntent(null, {
			source,
			systemAudio: true,
			micOn: false,
			micDeviceId: null,
			cameraOn: false,
			cameraName: null,
		});

		expect([rebuilt.targetType, rebuilt.targetId]).toEqual(["window", 9]);
	});
});

describe("deviceOutcome", () => {
	const name = (d: { name: string }) => d.name;
	const device = { name: "Built-in Mic" };

	it("turns a match on with no warning", () => {
		const outcome = deviceOutcome({ kind: "matched", device }, "Studio", "mic", name);

		expect([outcome.on, outcome.warning]).toEqual([true, null]);
	});

	it("keeps a fallback on", () => {
		const outcome = deviceOutcome(
			{ kind: "fallback", requestedLabel: "Yeti", device, reason: "gone" },
			"Studio",
			"mic",
			name,
		);

		expect(outcome.on).toBe(true);
	});

	it("names both devices in a fallback warning", () => {
		const outcome = deviceOutcome(
			{ kind: "fallback", requestedLabel: "Yeti", device, reason: "gone" },
			"Studio",
			"mic",
			name,
		);

		expect(outcome.warning).toBe("“Yeti” unavailable, using “Built-in Mic”");
	});

	it("turns a missing device off", () => {
		const outcome = deviceOutcome(
			{ kind: "missing", requestedLabel: "Yeti" },
			"Studio",
			"mic",
			name,
		);

		expect([outcome.on, outcome.device]).toEqual([false, null]);
	});

	it("names the profile and the device class when one is missing", () => {
		const outcome = deviceOutcome(
			{ kind: "missing", requestedLabel: "Yeti" },
			"Studio",
			"mic",
			name,
		);

		expect(outcome.warning).toBe("“Studio” wants a mic but none is available");
	});

	it("says nothing when the profile asked for no device", () => {
		const outcome = deviceOutcome({ kind: "none" }, "Studio", "camera", name);

		expect([outcome.on, outcome.warning]).toEqual([false, null]);
	});

	// The panel tears the camera preview down differently for the two, so the distinction has to survive.
	it("keeps missing and none distinguishable", () => {
		const missing = deviceOutcome({ kind: "missing", requestedLabel: "X" }, "P", "camera", name);
		const none = deviceOutcome({ kind: "none" }, "P", "camera", name);

		expect([missing.kind, none.kind]).toEqual(["missing", "none"]);
	});
});
