import type { EditorImage } from "./types";

/** Load a data/object URL into an `EditorImage`, resolving its intrinsic size.
 * Rejects if the source can't be decoded as an image. */
export function imageFromSrc(src: string): Promise<EditorImage> {
	return new Promise((resolve, reject) => {
		const img = new Image();
		img.onload = () => resolve({ src, width: img.naturalWidth, height: img.naturalHeight });
		img.onerror = () => reject(new Error("could not decode the image"));
		img.src = src;
	});
}

/** Read a `File`/`Blob` as a self-contained data URL, then resolve its size.
 * Data URLs (vs object URLs) need no revoke bookkeeping and survive an export
 * snapshot, which matters when the same node is serialized off-DOM. */
export function imageFromFile(file: Blob): Promise<EditorImage> {
	if (!file.type.startsWith("image/")) {
		return Promise.reject(new Error("that file is not an image"));
	}
	// Preserve the upload's name (sans extension) so exports can use it.
	const name = file instanceof File && file.name ? file.name.replace(/\.[^.]+$/, "") : undefined;
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => {
			const src = reader.result;
			if (typeof src !== "string") {
				reject(new Error("could not read the image"));
				return;
			}
			imageFromSrc(src).then((img) => resolve({ ...img, name }), reject);
		};
		reader.onerror = () => reject(new Error("could not read the file"));
		reader.readAsDataURL(file);
	});
}

/** First image found on a clipboard/drag `DataTransfer`, or null. */
export function imageFromDataTransfer(data: DataTransfer | null): File | null {
	if (!data) return null;
	for (const item of Array.from(data.items)) {
		if (item.kind === "file" && item.type.startsWith("image/")) {
			const file = item.getAsFile();
			if (file) return file;
		}
	}
	for (const file of Array.from(data.files)) {
		if (file.type.startsWith("image/")) return file;
	}
	return null;
}
