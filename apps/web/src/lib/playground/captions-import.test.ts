import { describe, expect, it } from "vitest";
import { parseSubtitles, parseTimestamp, transcriptToVtt } from "./captions-import";

describe("parseTimestamp", () => {
	it("reads both the SRT comma and the VTT dot", () => {
		expect(parseTimestamp("00:00:01,500")).toBe(1.5);
		expect(parseTimestamp("00:00:01.500")).toBe(1.5);
	});

	it("reads an hour-less VTT stamp", () => {
		expect(parseTimestamp("01:30.250")).toBe(90.25);
	});

	it("pads a short milliseconds field rather than misreading it", () => {
		expect(parseTimestamp("00:00:00.5")).toBe(0.5);
	});

	it("returns null for something that isn't a timestamp", () => {
		expect(parseTimestamp("later")).toBeNull();
		expect(parseTimestamp("")).toBeNull();
	});
});

describe("parseSubtitles", () => {
	const srt = `1
00:00:00,000 --> 00:00:02,000
Hello there

2
00:00:02,500 --> 00:00:04,000
Second cue
across two lines
`;

	it("parses SRT cues with their numbering stripped", () => {
		const t = parseSubtitles(srt, "clip.srt");
		expect(t.segments).toHaveLength(2);
		expect(t.segments[0]).toMatchObject({ start: 0, end: 2, text: "Hello there" });
		expect(t.segments[1].text).toBe("Second cue across two lines");
	});

	it("parses VTT, skipping the header, NOTE blocks and cue settings", () => {
		const vtt = `WEBVTT - my captions

NOTE this should be ignored

intro
00:00:01.000 --> 00:00:02.000 align:start position:10%
Styled cue
`;
		const t = parseSubtitles(vtt, "clip.vtt");
		expect(t.segments).toHaveLength(1);
		expect(t.segments[0]).toMatchObject({ start: 1, end: 2, text: "Styled cue" });
	});

	it("strips inline markup so tags don't render as caption text", () => {
		const t = parseSubtitles("00:00:00.000 --> 00:00:01.000\n<b>Bold</b> text\n");
		expect(t.segments[0].text).toBe("Bold text");
	});

	// One malformed cue must not cost the user the whole file.
	it("skips an unparseable or inverted cue and keeps the rest", () => {
		const t = parseSubtitles(`00:00:00,000 --> 00:00:01,000
good

not a timestamp --> nope
bad

00:00:05,000 --> 00:00:04,000
inverted

00:00:06,000 --> 00:00:07,000
also good
`);
		expect(t.segments.map((s: { text: string }) => s.text)).toEqual(["good", "also good"]);
	});

	it("handles CRLF and a BOM", () => {
		const t = parseSubtitles("﻿00:00:00,000 --> 00:00:01,000\r\nhi\r\n");
		expect(t.segments[0].text).toBe("hi");
	});

	it("returns an empty transcript rather than throwing on junk", () => {
		expect(parseSubtitles("this is not a subtitle file").segments).toEqual([]);
	});

	// Progressive-highlight caption styles need per-word timings to advance on.
	it("spreads word timings evenly across the cue", () => {
		const t = parseSubtitles("00:00:00,000 --> 00:00:04,000\none two three four\n");
		const words = t.segments[0].words;
		expect(words.map((w: { text: string }) => w.text)).toEqual(["one", "two", "three", "four"]);
		expect(words[0].start).toBe(0);
		expect(words[3].end).toBeCloseTo(4, 6);
		expect(words[1].start).toBeCloseTo(1, 6);
	});
});

describe("transcriptToVtt", () => {
	it("round-trips through the parser", () => {
		const original = parseSubtitles("00:00:01,250 --> 00:00:03,500\nround trip\n");
		const back = parseSubtitles(transcriptToVtt(original));
		expect(back.segments[0]).toMatchObject({ start: 1.25, end: 3.5, text: "round trip" });
	});

	it("emits a WEBVTT header", () => {
		expect(transcriptToVtt(parseSubtitles(""))).toMatch(/^WEBVTT/);
	});
});
