import type { EngineBackend, NavigatorLike } from "./types";

/**
 * Asks for a DEVICE, not just an adapter, and not `"gpu" in navigator`.
 *
 * Each step rules out a real machine. WebView2 and Linux Chrome expose
 * `navigator.gpu` where no adapter comes back. Headless Chromium on Windows
 * returns an adapter and then refuses the device, because Dawn cannot load
 * `dxil.dll`. That last one has no recovery after the fact: a canvas keeps the
 * first context type it is given, so a failed WebGPU surface leaves a canvas on
 * which `getContext("webgl2")` returns null. The probe has to be right BEFORE
 * the canvas is touched, which is why it pays for a device it throws away.
 */
export async function detectBackend(
	nav: NavigatorLike | undefined,
	requested: EngineBackend | "auto" = "auto",
): Promise<EngineBackend> {
	if (requested !== "auto") return requested;
	const gpu = nav?.gpu;
	if (!gpu) return "webgl2";
	try {
		const adapter = await gpu.requestAdapter();
		if (!adapter) return "webgl2";
		const device = await adapter.requestDevice();
		if (!device) return "webgl2";
		device.destroy?.();
		return "webgpu";
	} catch {
		return "webgl2";
	}
}
