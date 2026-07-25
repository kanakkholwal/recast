import { describe, expect, it } from "vitest";
import { buildJamendoSearchUrl, parseJamendoTracks } from "./jamendo";

describe("buildJamendoSearchUrl", () => {
	it("builds a /tracks query with the client id, format, and name search", () => {
		const url = buildJamendoSearchUrl("KEY", "lofi beats", 10);
		expect(url.startsWith("https://api.jamendo.com/v3.0/tracks/?")).toBe(true);
		const q = new URL(url).searchParams;
		expect(q.get("client_id")).toBe("KEY");
		expect(q.get("format")).toBe("json");
		expect(q.get("limit")).toBe("10");
		expect(q.get("namesearch")).toBe("lofi beats");
		expect(q.get("audioformat")).toBe("mp32");
	});
});

describe("parseJamendoTracks", () => {
	const sample = {
		results: [
			{
				id: 123,
				name: "Sunrise",
				artist_name: "Nova",
				duration: 180,
				audio: "https://cdn.jamendo/stream/123.mp3",
				audiodownload: "https://cdn.jamendo/dl/123.mp3",
				audiodownload_allowed: true,
				license_ccurl: "https://creativecommons.org/licenses/by/4.0/",
			},
			{
				id: 200,
				name: "Nightfall",
				artist_name: "Echo",
				audio: "https://cdn.jamendo/stream/200.mp3",
				audiodownload: "",
				audiodownload_allowed: false,
			},
		],
	};

	it("prefers the download URL when allowed, else the streaming URL", () => {
		const [a, b] = parseJamendoTracks(sample);
		expect(a.downloadUrl).toBe("https://cdn.jamendo/dl/123.mp3");
		expect(b.downloadUrl).toBe("https://cdn.jamendo/stream/200.mp3"); // dl not allowed → stream
	});

	it("keeps the streaming URL for audition (separate from download)", () => {
		const [a] = parseJamendoTracks(sample);
		expect(a.previewUrl).toBe("https://cdn.jamendo/stream/123.mp3");
	});

	it("carries attribution + license for crediting", () => {
		const [a] = parseJamendoTracks(sample);
		expect(a.trackId).toBe("123");
		expect(a.attribution).toContain("Sunrise");
		expect(a.attribution).toContain("Nova");
		expect(a.attribution).toContain("Jamendo");
		expect(a.license).toContain("creativecommons.org");
		expect(a.durationSec).toBe(180);
	});

	it("drops tracks with no playable url and tolerates junk", () => {
		expect(parseJamendoTracks({ results: [{ id: 1, name: "x" }] })).toHaveLength(0);
		expect(parseJamendoTracks(null)).toEqual([]);
		expect(parseJamendoTracks({})).toEqual([]);
	});
});
