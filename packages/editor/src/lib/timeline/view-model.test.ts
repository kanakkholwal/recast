import { describe, expect, it } from "vitest";
import { buildTimeMap } from "./time-map";
import { buildTimelineRows, type TimelineRow, type TimelineViewModelInput } from "./view-model";

const identity = buildTimeMap([{ origStart: 0, origEnd: 10, speed: 1 }]);

const input = (over: Partial<TimelineViewModelInput> = {}): TimelineViewModelInput => ({
	fps: 30,
	map: identity,
	videoName: "clip.mp4",
	segments: [],
	zoomRegions: [],
	annotations: [],
	captions: [],
	voiceClips: [],
	musicClips: [],
	...over,
});

function kinds(rows: TimelineRow[]): string[] {
	return rows.map((r) => r.kind);
}

describe("buildTimelineRows", () => {
	it("emits one row per type, empty types omitted", () => {
		const rows = buildTimelineRows(
			input({
				segments: [{ id: "v", start: 0, end: 4, label: "Clip" }],
				zoomRegions: [{ id: "z", start: 1, end: 3, label: "1.6x" }],
				voiceClips: [{ id: "au", start: 6, end: 9, label: "Voice" }],
			}),
		);
		// markup/caption/music absent -> no rows for them.
		expect(kinds(rows)).toEqual(["video", "zoom", "audio"]);
		expect(rows[0].label).toBe("clip.mp4");
		expect(rows[0].clips[0].duration).toBe(4 * 30);
	});

	it("lays every clip of a type side by side on that type's one row", () => {
		const rows = buildTimelineRows(
			input({
				segments: [
					{ id: "a", start: 0, end: 2, label: "Clip" },
					{ id: "b", start: 2, end: 5, label: "Clip" },
				],
			}),
		);
		expect(rows).toHaveLength(1);
		expect(rows[0].kind).toBe("video");
		expect(rows[0].clips).toHaveLength(2);
	});

	it("keeps voice and music as separate Audio-typed rows", () => {
		const rows = buildTimelineRows(
			input({
				voiceClips: [{ id: "v1", start: 0, end: 2, label: "Voice" }],
				musicClips: [{ id: "m1", start: 0, end: 5, label: "Song" }],
			}),
		);
		expect(kinds(rows)).toEqual(["audio", "audio"]);
		expect(rows[0].label).toBe("Voice");
		expect(rows[1].label).toBe("Music");
	});

	it("collapses every caption segment onto a single Captions row", () => {
		const rows = buildTimelineRows(
			input({
				captions: [
					{ id: "c1", start: 0, end: 1, label: "hi" },
					{ id: "c2", start: 1, end: 2, label: "there" },
				],
			}),
		);
		expect(rows).toHaveLength(1);
		expect(rows[0].kind).toBe("caption");
		expect(rows[0].clips).toHaveLength(2);
	});

	it("projects original-axis clips through cuts, output-axis clips untouched", () => {
		const map = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 5, origEnd: 8, speed: 1 },
		]);
		const rows = buildTimelineRows(
			input({
				map,
				zoomRegions: [{ id: "z", start: 5, end: 7, label: "z" }], // orig 5..7 -> output 2..4
				voiceClips: [{ id: "au", start: 2, end: 4, label: "v" }],
			}),
		);
		const zoom = rows.find((r) => r.kind === "zoom");
		const audio = rows.find((r) => r.kind === "audio");
		expect(zoom?.clips[0].start).toBe(2 * 30);
		expect(zoom?.clips[0].duration).toBe(2 * 30);
		expect(audio?.clips[0].start).toBe(2 * 30);
	});

	it("stacks time-overlapping clips onto distinct lanes", () => {
		const rows = buildTimelineRows(
			input({
				annotations: [
					{ id: "a", start: 0, end: 4, label: "a" },
					{ id: "b", start: 2, end: 6, label: "b" }, // overlaps a -> lane 1
					{ id: "c", start: 6, end: 8, label: "c" }, // clear of a -> back to lane 0
				],
			}),
		);
		const markup = rows.find((r) => r.kind === "markup");
		expect(markup?.laneCount).toBe(2);
		const lane = (id: string) => markup?.clips.find((c) => c.id === id)?.lane;
		expect(lane("a")).toBe(0);
		expect(lane("b")).toBe(1);
		expect(lane("c")).toBe(0);
	});

	it("leaves non-overlapping clips on a single lane", () => {
		const rows = buildTimelineRows(
			input({
				segments: [
					{ id: "a", start: 0, end: 2, label: "Clip" },
					{ id: "b", start: 2, end: 5, label: "Clip" },
				],
			}),
		);
		expect(rows[0].laneCount).toBe(1);
		expect(rows[0].clips.every((c) => c.lane === 0)).toBe(true);
	});

	it("emits a keyframe track row, projecting times through cuts", () => {
		const map = buildTimeMap([
			{ origStart: 0, origEnd: 2, speed: 1 },
			{ origStart: 5, origEnd: 8, speed: 1 },
		]);
		const rows = buildTimelineRows(
			input({
				map,
				tracks: [
					{
						id: "camera",
						source: "camera",
						label: "Camera",
						kind: "camera",
						times: [1, 6], // orig 1 -> out 1; orig 6 -> out 3
						selectedTime: 6,
					},
				],
			}),
		);
		const cam = rows.find((r) => r.id === "camera");
		expect(cam?.track?.source).toBe("camera");
		expect(cam?.clips).toHaveLength(0);
		expect(cam?.track?.keyframes.map((k) => k.frame)).toEqual([1 * 30, 3 * 30]);
		expect(cam?.track?.keyframes[1].selected).toBe(true);
	});

	it("carries selection, hidden and locked flags", () => {
		const rows = buildTimelineRows(
			input({
				zoomRegions: [{ id: "z", start: 1, end: 3, label: "z", selected: true, hidden: true }],
				annotations: [{ id: "a", start: 1, end: 2, label: "a", locked: true }],
			}),
		);
		const zoom = rows.find((r) => r.kind === "zoom")?.clips[0];
		const markup = rows.find((r) => r.kind === "markup")?.clips[0];
		expect(zoom?.selected).toBe(true);
		expect(zoom?.hidden).toBe(true);
		expect(markup?.locked).toBe(true);
	});
});
describe("axis of each row", () => {
	// `captionTranscript` is recording-timed: read as output seconds, a caption after a cut drew where the words were spoken, not where they are heard.
	it("places captions on the original axis, so a cut shifts them", () => {
		// 5s cut out of the head: original 10s is heard at output 5s.
		const cut = buildTimeMap([{ origStart: 5, origEnd: 20, speed: 1 }]);
		const rows = buildTimelineRows(
			input({
				map: cut,
				captions: [{ id: "c", start: 10, end: 12, label: "hello" }],
			}),
		);
		const caption = rows[0].clips[0];
		expect(caption.start).toBe(5 * 30);
		expect(caption.duration).toBe(2 * 30);
	});

	// Voice and music really are output-timed, so the same input must NOT move.
	it("leaves voice clips on the output axis", () => {
		const cut = buildTimeMap([{ origStart: 5, origEnd: 20, speed: 1 }]);
		const rows = buildTimelineRows(
			input({
				map: cut,
				voiceClips: [{ id: "v", start: 10, end: 12, label: "Voice" }],
			}),
		);
		expect(rows[0].clips[0].start).toBe(10 * 30);
	});
});
