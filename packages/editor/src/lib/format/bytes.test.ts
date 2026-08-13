import { describe, expect, it } from "vitest";
import { formatBytes } from "./bytes";

describe("formatBytes", () => {
	it("returns the zero label for falsy/negative", () => {
		expect(formatBytes(0)).toBe("0 B");
		expect(formatBytes(undefined)).toBe("0 B");
		expect(formatBytes(-1)).toBe("0 B");
		expect(formatBytes(0, { zeroLabel: "--" })).toBe("--");
	});
	it("separates the empty label from the zero label", () => {
		expect(formatBytes(null, { zeroLabel: "0 GB", emptyLabel: "Unlimited" })).toBe("Unlimited");
		expect(formatBytes(0, { zeroLabel: "0 GB", emptyLabel: "Unlimited" })).toBe("0 GB");
		expect(formatBytes(Number.POSITIVE_INFINITY, { emptyLabel: "Unlimited" })).toBe("Unlimited");
	});
	it("keeps bytes whole and scales up by 1024", () => {
		expect(formatBytes(512)).toBe("512 B");
		expect(formatBytes(1024)).toBe("1 KB");
		expect(formatBytes(1536)).toBe("1.5 KB");
		expect(formatBytes(1.4 * 1024 ** 3)).toBe("1.4 GB");
	});
	it("keeps one decimal below 100 and drops it above", () => {
		expect(formatBytes(15.4 * 1024 * 1024)).toBe("15.4 MB");
		expect(formatBytes(15 * 1024 * 1024)).toBe("15 MB");
		expect(formatBytes(191_000_000)).toBe("182 MB");
	});
	it("promotes the unit rather than printing 1024 of the smaller one", () => {
		expect(formatBytes(1023.7 * 1024 ** 2)).toBe("1 GB");
	});
});
