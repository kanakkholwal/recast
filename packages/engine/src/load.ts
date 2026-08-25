import type { EngineBackend, EngineModule } from "./types";

/** Each backend is a separate artifact, so these stay as two literal dynamic
 *  imports: a computed specifier would make the bundler ship both. */
export async function loadEngineModule(backend: EngineBackend): Promise<EngineModule> {
	if (backend === "webgpu") {
		const [module, wasm] = await Promise.all([
			import("../wasm/recast_engine_webgpu.js"),
			import("../wasm/recast_engine_webgpu_bg.wasm?url"),
		]);
		await module.default({ module_or_path: wasm.default });
		return module as unknown as EngineModule;
	}
	const [module, wasm] = await Promise.all([
		import("../wasm/recast_engine_webgl2.js"),
		import("../wasm/recast_engine_webgl2_bg.wasm?url"),
	]);
	await module.default({ module_or_path: wasm.default });
	return module as unknown as EngineModule;
}
