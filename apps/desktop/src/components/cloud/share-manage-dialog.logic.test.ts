import { describe, expect, it } from "vitest";
import { buildShareUpdate, type ShareUpdateInput, toVisibility } from "./share-manage-dialog.logic";

const base: ShareUpdateInput = {
	visibility: "public",
	initialVisibility: "public",
	removePassword: false,
	password: "",
	expiryDate: "",
	initialExpiry: "",
};

describe("toVisibility", () => {
	it("maps team → workspace and unknown → private", () => {
		expect(toVisibility("public")).toBe("public");
		expect(toVisibility("workspace")).toBe("workspace");
		expect(toVisibility("team")).toBe("workspace");
		expect(toVisibility("whatever")).toBe("private");
	});
});

describe("buildShareUpdate", () => {
	it("returns nothing when nothing changed", () => {
		expect(buildShareUpdate(base)).toEqual({});
	});

	it("emits visibility only when it differs", () => {
		expect(buildShareUpdate({ ...base, visibility: "private" })).toEqual({ visibility: "private" });
	});

	it('removePassword wins over a typed password and emits ""', () => {
		const opts = buildShareUpdate({
			...base,
			removePassword: true,
			password: "hunter2",
		});
		expect(opts.password).toBe("");
	});

	it("sets a trimmed password", () => {
		expect(buildShareUpdate({ ...base, password: "  secret  " }).password).toBe("secret");
	});

	it("omits password for a blank/whitespace field (unchanged)", () => {
		expect("password" in buildShareUpdate({ ...base, password: "   " })).toBe(false);
	});

	it("emits end-of-day ISO when expiry is set", () => {
		const opts = buildShareUpdate({
			...base,
			expiryDate: "2026-07-05",
		});
		// End-of-day local time → the ISO instant equals that local 23:59:59.
		expect(opts.expiresAt).toBe(new Date("2026-07-05T23:59:59").toISOString());
	});

	it('emits "" to clear expiry when cleared from a prior value', () => {
		const opts = buildShareUpdate({
			...base,
			expiryDate: "",
			initialExpiry: "2026-07-05",
		});
		expect(opts.expiresAt).toBe("");
	});

	it("omits expiry when unchanged", () => {
		const opts = buildShareUpdate({
			...base,
			expiryDate: "2026-07-05",
			initialExpiry: "2026-07-05",
		});
		expect("expiresAt" in opts).toBe(false);
	});
});
