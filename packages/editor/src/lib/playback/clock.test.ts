import { beforeEach, describe, expect, it } from "vitest";
import { PlaybackClock } from "./clock";

/** Controllable time source so the integrator is tested without real time. */
function fakeNow() {
	let t = 1000; // arbitrary non-zero start (ms)
	const fn = () => t;
	fn.advance = (ms: number) => {
		t += ms;
	};
	fn.set = (ms: number) => {
		t = ms;
	};
	return fn;
}

describe("PlaybackClock", () => {
	let now: ReturnType<typeof fakeNow>;
	let clock: PlaybackClock;

	beforeEach(() => {
		now = fakeNow();
		clock = new PlaybackClock(now);
		clock.setDuration(10);
	});

	it("starts paused at 0", () => {
		expect(clock.playing).toBe(false);
		expect(clock.time).toBe(0);
	});

	it("does not advance while paused", () => {
		now.advance(5000);
		expect(clock.time).toBe(0);
	});

	it("advances with wall-clock at rate 1 while playing", () => {
		clock.play();
		now.advance(2500);
		expect(clock.time).toBeCloseTo(2.5, 6);
		now.advance(1000);
		expect(clock.time).toBeCloseTo(3.5, 6);
	});

	it("freezes at the current time on pause", () => {
		clock.play();
		now.advance(3000);
		clock.pause();
		const frozen = clock.time;
		expect(frozen).toBeCloseTo(3, 6);
		now.advance(5000);
		expect(clock.time).toBe(frozen);
	});

	it("resumes from the paused time", () => {
		clock.play();
		now.advance(3000);
		clock.pause();
		now.advance(9999); // paused gap doesn't count
		clock.play();
		now.advance(1000);
		expect(clock.time).toBeCloseTo(4, 6);
	});

	it("clamps to [0, duration]", () => {
		clock.play();
		now.advance(999_000);
		expect(clock.time).toBe(10);
		expect(clock.atEnd).toBe(true);
	});

	it("seek preserves play state and continues from the target", () => {
		clock.play();
		now.advance(1000);
		clock.seek(7);
		expect(clock.time).toBeCloseTo(7, 6);
		now.advance(1000);
		expect(clock.time).toBeCloseTo(8, 6);
	});

	it("seek while paused holds the target", () => {
		clock.seek(4);
		expect(clock.playing).toBe(false);
		expect(clock.time).toBeCloseTo(4, 6);
		now.advance(5000);
		expect(clock.time).toBeCloseTo(4, 6);
	});

	it("seek clamps out-of-range targets", () => {
		clock.seek(-3);
		expect(clock.time).toBe(0);
		clock.seek(50);
		expect(clock.time).toBe(10);
	});

	it("applies playback rate", () => {
		clock.setRate(2);
		clock.play();
		now.advance(1000);
		expect(clock.time).toBeCloseTo(2, 6);
	});

	it("rate change mid-playback is seamless (no retroactive jump)", () => {
		clock.play();
		now.advance(2000); // at 2s, rate 1
		clock.setRate(0.5);
		expect(clock.time).toBeCloseTo(2, 6);
		now.advance(2000); // +1s at half speed
		expect(clock.time).toBeCloseTo(3, 6);
	});

	it("ignores invalid rate values", () => {
		clock.setRate(0);
		clock.setRate(-1);
		clock.setRate(NaN);
		expect(clock.rate).toBe(1);
	});

	it("re-clamps the current time when duration shrinks below it", () => {
		clock.seek(8);
		clock.setDuration(5);
		expect(clock.time).toBe(5);
	});

	it("duration change while playing does not move the current time", () => {
		clock.play();
		now.advance(3000); // at 3s
		clock.setDuration(20);
		expect(clock.time).toBeCloseTo(3, 6);
	});

	it("treats a non-positive duration as zero", () => {
		clock.setDuration(-2);
		expect(clock.duration).toBe(0);
		clock.play();
		now.advance(1000);
		expect(clock.time).toBe(0);
	});
});
