import { loadEngineModule } from "./load";
import { detectBackend } from "./probe";
import type { EngineBackend, NavigatorLike, WasmProjectDocument } from "./types";

/**
 * The project-document parser from the engine module. Loads the same artifact
 * the preview uses (initialising a wasm module twice is a no-op), so a page
 * with a preview pays nothing extra for its document.
 */
export async function loadProjectDocumentParser(
	nav: NavigatorLike = globalThis.navigator as NavigatorLike,
	backend: EngineBackend | "auto" = "auto",
): Promise<(text: string) => WasmProjectDocument> {
	const chosen = await detectBackend(nav, backend);
	const module = await loadEngineModule(chosen);
	return (text) => module.ProjectDocument.parse(text);
}
