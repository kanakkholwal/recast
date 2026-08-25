import { parseColor } from "@recast/ui/color-picker/logic";
import { describe, expect, it } from "vitest";
import fixtures from "./__fixtures__/color-parity.json";
import { parseGradient } from "./render-state";

type Rgba = [number, number, number, number];

function asRgba(hex: string): Rgba | null {
	const parsed = parseColor(hex);
	if (!parsed) return null;
	return [parsed.r, parsed.g, parsed.b, Math.round(parsed.a * 255)];
}

describe("colour parsing agrees with recast-color", () => {
	for (const c of fixtures.colors) {
		it(`matches fixture: ${c.name}`, () => {
			expect(asRgba(c.input)).toEqual(c.expected);
		});
	}
});

describe("gradient parsing agrees with recast-color", () => {
	for (const g of fixtures.gradients) {
		it(`matches fixture: ${g.name}`, () => {
			const parsed = parseGradient(g.input);
			expect(parsed.angle).toBeCloseTo(g.angle, 6);
			expect(parsed.stops.length).toBe(g.stops.length);
			parsed.stops.forEach((stop, i) => {
				expect(asRgba(stop.color)).toEqual(g.stops[i].color);
				expect(stop.pos).toBeCloseTo(g.stops[i].pos, 6);
			});
		});
	}
});
