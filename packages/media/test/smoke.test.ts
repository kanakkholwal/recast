import { describe, expect, it } from "vitest";
import { MediaError } from "../src/errors";

/**
 * Surface-level smoke tests for the public API. The package's other tests
 * exercise real behavior; these tests guard against accidental signature
 * drift while the implementations land in later PRs.
 *
 * Contract (REQUIREMENTS.md §5): every async export takes an `AbortSignal`,
 * every error is a `MediaError` (never a plain `Error`), cancellation is a
 * `MediaError` with `code: 'cancelled'`.
 */
describe("public API surface", () => {
	it("MediaError carries a code and is recognized by `isCancelled`", () => {
		const e = new MediaError("cancelled", "Cancelled.");
		expect(e.code).toBe("cancelled");
		expect(e.isCancelled).toBe(true);
		expect(e.name).toBe("MediaError");
	});

	it("MediaError.isCancelled is false for non-cancellation codes", () => {
		expect(new MediaError("bad-input", "x").isCancelled).toBe(false);
		expect(new MediaError("unsupported", "x").isCancelled).toBe(false);
		expect(new MediaError("internal", "x").isCancelled).toBe(false);
	});

	it("MediaError preserves `cause`", () => {
		const root = new Error("boom");
		const wrapped = new MediaError("decode-failed", "decode failed", { cause: root });
		expect(wrapped.cause).toBe(root);
	});
});
