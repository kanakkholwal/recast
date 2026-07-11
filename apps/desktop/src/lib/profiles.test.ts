import { describe, expect, it } from "vitest";
import {
	capSig,
	ensureExactlyOneDefault,
	findDefaultProfile,
	reconcileProfileHydration,
	type RecordingProfile,
} from "./profiles";

function profile(over: Partial<RecordingProfile> = {}): RecordingProfile {
	return {
		id: "p1",
		name: "Profile",
		systemAudio: true,
		microphone: false,
		micDeviceId: null,
		micLabel: null,
		camera: false,
		cameraLabel: null,
		cameraDeviceId: null,
		countdown: null,
		isDefault: false,
		...over,
	};
}

describe("reconcileProfileHydration (localStorage -> backend migration)", () => {
	it("adopts the backend set when it is already initialized (no push)", () => {
		const backend = {
			profiles: [profile({ id: "b1", name: "Backend", isDefault: true })],
			enabled: false,
			initialized: true,
		};
		const legacy = { profiles: [profile({ id: "c1", isDefault: true })], enabled: true };
		const out = reconcileProfileHydration(backend, legacy);
		expect(out.push).toBe(false);
		expect(out.profiles.map((p) => p.id)).toEqual(["b1"]);
		expect(out.enabled).toBe(false);
	});

	it("migrates the legacy set up when the backend only holds the seed", () => {
		const backend = {
			profiles: [profile({ id: "seed", name: "Seed", isDefault: true })],
			enabled: true,
			initialized: false,
		};
		const legacy = {
			profiles: [
				profile({ id: "c1", name: "Mine", isDefault: true }),
				profile({ id: "c2", name: "Other" }),
			],
			enabled: false,
		};
		const out = reconcileProfileHydration(backend, legacy);
		expect(out.push).toBe(true);
		expect(out.profiles.map((p) => p.id)).toEqual(["c1", "c2"]);
		// Migrate preserves the legacy enabled flag, not the backend default.
		expect(out.enabled).toBe(false);
	});

	it("persists the backend seed on a fresh install (no legacy key)", () => {
		const backend = {
			profiles: [profile({ id: "seed", name: "Seed", isDefault: true })],
			enabled: true,
			initialized: false,
		};
		const out = reconcileProfileHydration(backend, null);
		expect(out.push).toBe(true);
		expect(out.profiles.map((p) => p.id)).toEqual(["seed"]);
	});

	it("repairs a broken default invariant when adopting", () => {
		const backend = {
			profiles: [
				profile({ id: "a", isDefault: false }),
				profile({ id: "b", isDefault: false }),
			],
			enabled: true,
			initialized: true,
		};
		const out = reconcileProfileHydration(backend, null);
		expect(out.profiles.filter((p) => p.isDefault)).toHaveLength(1);
		expect(out.profiles[0].isDefault).toBe(true);
	});
});

describe("default invariant (guards migration round-trips)", () => {
	it("promotes the first profile when none is default", () => {
		const list = ensureExactlyOneDefault([
			profile({ id: "a", isDefault: false }),
			profile({ id: "b", isDefault: false }),
		]);
		expect(list.filter((p) => p.isDefault)).toHaveLength(1);
		expect(list[0].isDefault).toBe(true);
	});

	it("keeps only the first of several defaults", () => {
		const list = ensureExactlyOneDefault([
			profile({ id: "a", isDefault: true }),
			profile({ id: "b", isDefault: true }),
		]);
		expect(list.filter((p) => p.isDefault)).toHaveLength(1);
		expect(list.find((p) => p.isDefault)?.id).toBe("a");
	});

	it("finds the default, falling back to the first", () => {
		expect(findDefaultProfile([])).toBeNull();
		expect(
			findDefaultProfile([profile({ id: "a" }), profile({ id: "b", isDefault: true })])?.id,
		).toBe("b");
		expect(findDefaultProfile([profile({ id: "a" })])?.id).toBe("a");
	});
});

describe("capability signature (dedup key survives the round trip)", () => {
	it("is stable for the same capture shape and device pointers", () => {
		const a = profile({ id: "a", microphone: true, micDeviceId: "mic-1" });
		const b = profile({ id: "b", microphone: true, micDeviceId: "mic-1" });
		expect(capSig(a)).toBe(capSig(b));
	});

	it("differs when a device pointer differs", () => {
		const a = profile({ microphone: true, micDeviceId: "mic-1" });
		const b = profile({ microphone: true, micDeviceId: "mic-2" });
		expect(capSig(a)).not.toBe(capSig(b));
	});
});
