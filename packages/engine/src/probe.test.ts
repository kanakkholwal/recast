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

	it("picks webgpu only once a device actually comes back", async () => {
		const destroy = vi.fn();
		const gpu = { requestAdapter: async () => ({ requestDevice: async () => ({ destroy }) }) };
		await expect(detectBackend({ gpu })).resolves.toBe("webgpu");
		expect(destroy, "the probe's device must not outlive the probe").toHaveBeenCalled();
	});

	/** Headless Chromium on Windows hands back an adapter and then refuses the
	 *  device (Dawn cannot load dxil.dll). There is no recovering from this after
	 *  the canvas is claimed, so the probe has to catch it. */
	it("falls back when an adapter comes back but the device request fails", async () => {
		const throwing = {
			requestAdapter: async () => ({
				requestDevice: async () => {
					throw new Error("dxil.dll");
				},
			}),
		};
		await expect(detectBackend({ gpu: throwing })).resolves.toBe("webgl2");

		const empty = { requestAdapter: async () => ({ requestDevice: async () => null }) };
		await expect(detectBackend({ gpu: empty })).resolves.toBe("webgl2");
	});

	/** WebView2 and Linux Chrome expose `navigator.gpu` on machines that hand
	 *  back no adapter, so presence alone would pick a backend that cannot run. */
	it("falls back when navigator.gpu is present but yields no adapter", async () => {
		await expect(detectBackend({ gpu: { requestAdapter: async () => null } })).resolves.toBe(
			"webgl2",
		);
	});

	it("survives a device with no destroy at all", async () => {
		const gpu = { requestAdapter: async () => ({ requestDevice: async () => ({}) }) };
		await expect(detectBackend({ gpu })).resolves.toBe("webgpu");
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
