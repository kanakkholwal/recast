import { describe, expect, it } from "vitest";
import { reconcileAvDrift } from "./av-drift";

const SYNC = 0.12;
const MAX_LEAD = 0.5;

function act(audioTime: number, pictureTime: number, isJump = false) {
	return reconcileAvDrift({
		audioTime,
		pictureTime,
		isJump,
		syncThreshold: SYNC,
		maxLead: MAX_LEAD,
	});
}

describe("reconcileAvDrift", () => {
	it("snaps audio on an intentional jump, either direction", () => {
		expect(act(1.0, 5.0, true)).toBe("resync-audio"); // cut skip / scrub fwd
		expect(act(5.0, 1.0, true)).toBe("resync-audio"); // scrub back
	});

	it("nudges audio forward when it falls behind the picture", () => {
		// audio 1.0, picture 1.3 → audio 0.3s behind.
		expect(act(1.0, 1.3)).toBe("resync-audio");
	});

	it("does NOT rewind audio to a stalled picture (the echo case)", () => {
		// Picture stalled at 1.0 while audio played to 1.4 (0.4s ahead, under
		// maxLead). Rewinding replays 0.4s of audio = the live echo. Leave it.
		expect(act(1.4, 1.0)).toBe("none");
	});

	it("advances the picture when it stalls far behind the audio", () => {
		// Audio 1.6, picture 1.0 → 0.6s lead (> maxLead). Catch the picture up
		// instead of rewinding audio: no echo, and the lead can't accumulate.
		expect(act(1.6, 1.0)).toBe("catch-picture");
	});

	it("tolerates sub-threshold drift in both directions", () => {
		expect(act(1.0, 1.05)).toBe("none"); // slightly behind
		expect(act(1.05, 1.0)).toBe("none"); // slightly ahead
	});

	it("crosses each boundary in the right direction", () => {
		expect(act(1.0, 1.0 + SYNC * 1.5)).toBe("resync-audio"); // clearly behind
		expect(act(1.0 + MAX_LEAD * 1.2, 1.0)).toBe("catch-picture"); // clearly ahead
		expect(act(1.0 + MAX_LEAD * 0.5, 1.0)).toBe("none"); // ahead but within lead
	});
});
