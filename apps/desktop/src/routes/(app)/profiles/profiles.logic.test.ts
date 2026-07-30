import { describe, expect, it } from "vitest";
import type { RecordingProfile } from "$lib/profiles";
import { isDraftDirty, nameClashOf, normalizeProfileForSave } from "./profiles.logic";

function profile(over: Partial<RecordingProfile> = {}): RecordingProfile {
	return {
		id: "a",
		name: "Meeting",
		systemAudio: true,
		microphone: false,
		micDeviceId: null,
		micLabel: null,
		camera: false,
		cameraDeviceId: null,
		cameraLabel: null,
		countdown: null,
		isDefault: false,
		...over,
	};
}

describe("isDraftDirty", () => {
	it("is clean against an untouched copy", () => {
		const p = profile();
		expect(isDraftDirty({ ...p }, p)).toBe(false);
	});

	it("sees every field the form can edit", () => {
		const p = profile();
		const edits: Partial<RecordingProfile>[] = [
			{ name: "Standup" },
			{ systemAudio: false },
			{ microphone: true },
			{ camera: true },
			{ countdown: 3 },
			{ isDefault: true },
		];
		for (const edit of edits) {
			expect(isDraftDirty(profile(edit), p)).toBe(true);
		}
	});

	// Only prompt about losing something real: whitespace and a device pointer
	// that save would strip anyway are not changes worth a dialog.
	it("ignores name whitespace", () => {
		expect(isDraftDirty(profile({ name: "  Meeting  " }), profile())).toBe(false);
	});

	it("ignores a device pointer under a disabled capability", () => {
		const p = profile({ microphone: false });
		expect(isDraftDirty(profile({ microphone: false, micDeviceId: "mic-1" }), p)).toBe(false);
	});

	it("sees a device swap while the capability is on", () => {
		const p = profile({ microphone: true, micDeviceId: "mic-1" });
		expect(isDraftDirty(profile({ microphone: true, micDeviceId: "mic-2" }), p)).toBe(true);
	});
});

describe("nameClashOf", () => {
	const others = [profile({ id: "b", name: "Standup" })];

	it("finds a clash regardless of case or padding", () => {
		expect(nameClashOf(profile({ name: " standup " }), others)?.id).toBe("b");
	});

	it("does not flag the profile against itself", () => {
		expect(nameClashOf(profile({ id: "b", name: "Standup" }), others)).toBeNull();
	});

	it("returns null for a unique or empty name", () => {
		expect(nameClashOf(profile({ name: "Demo" }), others)).toBeNull();
		expect(nameClashOf(profile({ name: "   " }), others)).toBeNull();
	});
});

describe("normalizeProfileForSave", () => {
	it("drops device pointers for disabled capabilities", () => {
		const next = normalizeProfileForSave(
			profile({ microphone: false, micDeviceId: "m", camera: false, cameraDeviceId: "c" }),
		);
		expect(next.micDeviceId).toBeNull();
		expect(next.cameraDeviceId).toBeNull();
	});

	it("keeps them when the capability is on", () => {
		const next = normalizeProfileForSave(profile({ microphone: true, micDeviceId: "m" }));
		expect(next.micDeviceId).toBe("m");
	});
});
