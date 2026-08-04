import { describe, expect, it } from "vitest";
import {
	imagePreviewSrc,
	isValidImageValue,
	lerpHex,
	sampleStopColor,
} from "./background-picker.logic";

describe("lerpHex", () => {
	it("returns the endpoints at f=0 and f=1", () => {
		expect(lerpHex("#000000", "#ffffff", 0)).toBe("#000000");
		expect(lerpHex("#000000", "#ffffff", 1)).toBe("#ffffff");
	});
	it("interpolates the midpoint per channel", () => {
		expect(lerpHex("#000000", "#ffffff", 0.5)).toBe("#808080");
		expect(lerpHex("#ff0000", "#0000ff", 0.5)).toBe("#800080");
	});
});

describe("sampleStopColor", () => {
	const stops = [
		{ color: "#000000", pos: 0 },
		{ color: "#ffffff", pos: 100 },
	];
	it("clamps to the nearest endpoint outside the range", () => {
		expect(sampleStopColor(stops, -10)).toBe("#000000");
		expect(sampleStopColor(stops, 200)).toBe("#ffffff");
	});
	it("interpolates between surrounding stops", () => {
		expect(sampleStopColor(stops, 50)).toBe("#808080");
	});
	it("works regardless of stop order", () => {
		const unsorted = [
			{ color: "#ffffff", pos: 100 },
			{ color: "#000000", pos: 0 },
		];
		expect(sampleStopColor(unsorted, 50)).toBe("#808080");
	});
});

describe("isValidImageValue", () => {
	it("rejects gradient/colour/asset leftovers and empties", () => {
		expect(isValidImageValue("")).toBe(false);
		expect(isValidImageValue("linear-gradient(...)")).toBe(false);
		expect(isValidImageValue("#ff0000")).toBe(false);
		expect(isValidImageValue("asset:abc")).toBe(false);
	});
	it("accepts direct sources and image extensions", () => {
		expect(isValidImageValue("https://x/y.png")).toBe(true);
		expect(isValidImageValue("data:image/png;base64,AAA")).toBe(true);
		expect(isValidImageValue("/wallpapers/foo")).toBe(true);
		expect(isValidImageValue("C:/pics/shot.webp")).toBe(true);
	});
	it("rejects a bare path with no image extension", () => {
		expect(isValidImageValue("C:/pics/shot")).toBe(false);
	});
});

describe("imagePreviewSrc", () => {
	const resolve = (v: string) => `asset://localhost/${v}`;
	it("returns empty for non-image leftovers", () => {
		expect(imagePreviewSrc("#fff", resolve)).toBe("");
		expect(imagePreviewSrc("", resolve)).toBe("");
	});
	it("returns direct sources unchanged", () => {
		expect(imagePreviewSrc("https://x/y.png", resolve)).toBe("https://x/y.png");
	});
	it("resolves a file path (even without an image extension)", () => {
		expect(imagePreviewSrc("C:/pics/shot", resolve)).toBe("asset://localhost/C:/pics/shot");
	});
});
