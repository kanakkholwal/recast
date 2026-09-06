import { describe, expect, it } from "vitest";
import { canReportCount, libraryStatus } from "./status";

const base = { loading: false, error: null, total: 0, matches: 0, query: "" };

describe("libraryStatus", () => {
	it("shows a skeleton only on the first load", () => {
		expect(libraryStatus({ ...base, loading: true })).toBe("loading");
		// A refresh over an existing list keeps the list rather than blanking it.
		expect(libraryStatus({ ...base, loading: true, total: 4, matches: 4 })).toBe("ready");
	});

	// The bug: a failed scan left entries empty and the page said 'No recordings yet', which reads as an empty disk.
	it("separates a failed scan from an empty one", () => {
		expect(libraryStatus({ ...base, error: "boom" })).toBe("error");
		expect(libraryStatus({ ...base })).toBe("empty");
	});

	it("keeps showing results when a refresh fails", () => {
		expect(libraryStatus({ ...base, error: "boom", total: 2, matches: 2 })).toBe("ready");
	});

	it("distinguishes an empty library from a search with no hits", () => {
		expect(libraryStatus({ ...base, total: 5, matches: 0, query: "zzz" })).toBe("no-matches");
		expect(libraryStatus({ ...base, total: 0, matches: 0, query: "zzz" })).toBe("empty");
	});

	// A whitespace-only query isn't a search, so its zero results must not offer a Clear search button.
	it("treats a blank query as no query", () => {
		expect(libraryStatus({ ...base, total: 5, matches: 0, query: "   " })).toBe("empty");
	});

	it("is ready when matches exist", () => {
		expect(libraryStatus({ ...base, total: 5, matches: 2, query: "a" })).toBe("ready");
	});
});

describe("canReportCount", () => {
	it("withholds a count until the list is known", () => {
		expect(canReportCount("loading")).toBe(false);
		expect(canReportCount("error")).toBe(false);
	});

	it("allows a count once it is", () => {
		for (const s of ["empty", "no-matches", "ready"] as const) {
			expect(canReportCount(s)).toBe(true);
		}
	});
});
