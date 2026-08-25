import type { EngineBackend, EngineModule } from "./types";

/**
 * Each backend is a separate artifact, so these stay as two literal dynamic
 * imports: a computed specifier would make the bundler ship both. The wasm URL
 * comes from `new URL(..., import.meta.url)` rather than a `?url` import, which
 * needs a bundler-specific module declaration to type-check.
 */
export async function loadEngineModule(backend: EngineBackend): Promise<EngineModule> {
	if (backend === "webgpu") {
		const module = await import("../wasm/recast_engine_webgpu.js");
		await module.default({
			module_or_path: new URL("../wasm/recast_engine_webgpu_bg.wasm", import.meta.url).href,
		});
		return module as unknown as EngineModule;
	}
	const module = await import("../wasm/recast_engine_webgl2.js");
	await module.default({
		module_or_path: new URL("../wasm/recast_engine_webgl2_bg.wasm", import.meta.url).href,
	});
	return module as unknown as EngineModule;
}
