import { describe, expect, it } from "vitest";
import type { ZoomRegionLike } from "../../lib/annotations/eval";
import { annotationZoom } from "./annotation-projection.logic";

const LINEAR = { x1: 0, y1: 0, x2: 1, y2: 1 };
const ZOOM: ZoomRegionLike[] = [
	{
		start: 0,
		end: 10,
		scale: 2,
		rampIn: 0,
		rampOut: 0,
		easeIn: LINEAR,
		easeOut: LINEAR,
		centerX: 0.25,
		centerY: 0.25,
	},
];

describe("annotationZoom", () => {
	it("tracks an active zoom for video-anchored markup", () => {
		expect(annotationZoom("video", ZOOM, 5, true).scale).toBeGreaterThan(1);
	});

	it("ignores zoom for frame-anchored markup", () => {
		expect(annotationZoom("frame", ZOOM, 5, true).scale).toBe(1);
	});

	// With focus off the composite stops zooming and the export drops the regions, so projecting markup through zoom misplaces it.
	it("ignores zoom for video-anchored markup when focus is disabled", () => {
		expect(annotationZoom("video", ZOOM, 5, false).scale).toBe(1);
	});

	it("treats a missing anchor as video-anchored", () => {
		expect(annotationZoom(undefined, ZOOM, 5, true).scale).toBeGreaterThan(1);
		expect(annotationZoom(undefined, ZOOM, 5, false).scale).toBe(1);
	});
});
