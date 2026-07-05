import { convertFileSrc } from "@tauri-apps/api/core";
import type { AnnotationKind } from "$lib/stores/editor-store.svelte";
import { fitImageBox } from "./resize-constraints";

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

/** Open an image file picker. Returns the absolute path, or null if cancelled. */
export async function pickImageFile(): Promise<string | null> {
	const { open } = await import("@tauri-apps/plugin-dialog");
	const selected = await open({
		multiple: false,
		directory: false,
		title: "Insert Image",
		filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
	});
	return typeof selected === "string" ? selected : null;
}

/**
 * Open a file picker and build a centered, aspect-correct image annotation kind.
 * Returns null if the user cancelled. `frameAspect` = video width / height (px).
 * The absolute path is stored on `path`; the export pipeline decodes it directly
 * and the preview loads it through `convertFileSrc`.
 */
export async function pickImageAnnotation(
	frameAspect: number,
): Promise<ImageAnnotationKind | null> {
	const selected = await pickImageFile();
	if (!selected) return null;

	const natural = await loadNaturalSize(convertFileSrc(selected));
	const box = fitImageBox(natural, frameAspect > 0 ? frameAspect : 16 / 9);
	return { kind: "image", ...box, path: selected, opacity: 1, radius: 0 };
}
