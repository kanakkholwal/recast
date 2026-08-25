import type { EngineBackend, NavigatorLike } from "./types";

/** Asks for an adapter rather than trusting `"gpu" in navigator`: WebView2 and
 *  Linux Chrome both expose the object on machines where no adapter comes back. */
export async function detectBackend(
	nav: NavigatorLike | undefined,
	requested: EngineBackend | "auto" = "auto",
): Promise<EngineBackend> {
	if (requested !== "auto") return requested;
	const gpu = nav?.gpu;
	if (!gpu) return "webgl2";
	try {
		return (await gpu.requestAdapter()) ? "webgpu" : "webgl2";
	} catch {
		return "webgl2";
	}
}
