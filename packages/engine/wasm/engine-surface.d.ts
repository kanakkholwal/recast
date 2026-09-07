import type { WasmPreviewEngine } from "../src/types";

/** Hand-written because wasm-bindgen's own `.d.ts` carries per-build codegen
 *  hashes. Must match the `#[wasm_bindgen]` surface in `crates/recast-ffi-wasm`. */
export declare const PreviewEngine: {
	create(canvas: unknown, backend?: string | null): Promise<WasmPreviewEngine>;
};

export default function init(options: { module_or_path: string }): Promise<unknown>;
