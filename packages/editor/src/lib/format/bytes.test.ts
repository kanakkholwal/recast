import { describe, expect, it } from "vitest";
import { formatBytes } from "./bytes";

describe("formatBytes", () => {
	it("returns the zero label for falsy/negative", () => {
		expect(formatBytes(0)).toBe("0 B");
		expect(formatBytes(undefined)).toBe("0 B");
		expect(formatBytes(-1)).toBe("0 B");
		expect(formatBytes(0, "--")).toBe("--");
	});
	it("keeps bytes whole and scales up by 1024", () => {
		expect(formatBytes(512)).toBe("512 B");
		expect(formatBytes(1024)).toBe("1.0 KB");
		expect(formatBytes(1536)).toBe("1.5 KB");
		expect(formatBytes(1048576)).toBe("1.0 MB");
		expect(formatBytes(1.4 * 1024 ** 3)).toBe("1.4 GB");
	});
	it("drops the decimal at/above 10 of a unit", () => {
		expect(formatBytes(15 * 1024 * 1024)).toBe("15 MB");
	});
});
