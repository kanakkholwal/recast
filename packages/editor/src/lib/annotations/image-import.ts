import { toast } from "@recast/ui/sonner";
import { getEditorServices } from "../editor/services";
import type { AnnotationKind, EditorStore } from "../../stores/editor-store.svelte";
import { fitImageBox } from "./resize-constraints";

const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif"];

export type ImageAnnotationKind = Extract<AnnotationKind, { kind: "image" }>;

/** Natural pixel size of an image URL, or null if it can't be measured. */
function loadNaturalSize(src: string): Promise<{ w: number; h: number } | null> {
	return new Promise((resolve) => {
		const img = new Image();
		img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
		img.onerror = () => resolve(null);
		img.src = src;
	});
}

/** Open an image file picker. Returns the asset ref, or null if the host has no
 *  picker or the user cancelled. */
export async function pickImageFile(): Promise<string | null> {
	const pick = getEditorServices().pickFile;
	if (!pick) return null;
	return await pick({ accept: IMAGE_EXTENSIONS, title: "Insert Image" });
}

/**
 * Open a file picker and build a centered, aspect-correct image annotation kind.
 * Returns null if the user cancelled. `frameAspect` = video width / height (px).
 * The ref is stored on `path`; the export pipeline decodes it directly and the
 * preview loads it through the host's resolver.
 */
export async function pickImageAnnotation(
	frameAspect: number,
): Promise<ImageAnnotationKind | null> {
	const selected = await pickImageFile();
	if (!selected) return null;

	const natural = await loadNaturalSize(getEditorServices().resolveAssetUrl(selected));
	const box = fitImageBox(natural, frameAspect > 0 ? frameAspect : 16 / 9);
	return { kind: "image", ...box, path: selected, opacity: 1, radius: 0 };
}

/** Pick + insert in one step, for the two surfaces that offer Insert image (the
 *  markup panel and the canvas toolbar). Disarms any tool first: an insert is a
 *  one-shot action, so leaving a drawing mode armed behind it is a trap. */
export async function insertImageAnnotation(store: EditorStore): Promise<void> {
	store.annotationTool = null;
	const meta = store.metadata;
	const frameAspect = meta && meta.height > 0 ? meta.width / meta.height : 16 / 9;
	try {
		const kind = await pickImageAnnotation(frameAspect);
		if (kind) store.addAnnotation(kind);
	} catch (error) {
		toast.error(`Could not insert image: ${error}`);
	}
}
