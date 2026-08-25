import { describe, expect, it, vi } from "vitest";
import { detectBackend } from "./probe";

describe("detectBackend", () => {
	it("takes an explicit choice without probing at all", async () => {
		const requestAdapter = vi.fn();
		await expect(detectBackend({ gpu: { requestAdapter } }, "webgl2")).resolves.toBe("webgl2");
		await expect(detectBackend(undefined, "webgpu")).resolves.toBe("webgpu");
		expect(requestAdapter).not.toHaveBeenCalled();
	});

	it("falls back to webgl2 when the browser has no webgpu at all", async () => {
		await expect(detectBackend({})).resolves.toBe("webgl2");
		await expect(detectBackend(undefined)).resolves.toBe("webgl2");
	});

	it("picks webgpu only once an adapter actually comes back", async () => {
		const adapter = detectBackend({ gpu: { requestAdapter: async () => ({}) } });
		await expect(adapter).resolves.toBe("webgpu");
	});

	/** WebView2 and Linux Chrome expose `navigator.gpu` on machines that hand
	 *  back no adapter, so presence alone would pick a backend that cannot run. */
	it("falls back when navigator.gpu is present but yields no adapter", async () => {
		await expect(detectBackend({ gpu: { requestAdapter: async () => null } })).resolves.toBe(
			"webgl2",
		);
	});

	it("falls back rather than rejecting when the adapter request throws", async () => {
		const gpu = {
			requestAdapter: async () => {
				throw new Error("no");
			},
		};
		await expect(detectBackend({ gpu })).resolves.toBe("webgl2");
	});
});
