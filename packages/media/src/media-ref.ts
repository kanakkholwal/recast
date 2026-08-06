/**
 * Where media bytes come from, for every code path that opens a MediaBunny
 * `Input`: a URL (range-requested over the network / Tauri asset protocol) or a
 * `Blob`/`File` (sliced straight off disk).
 *
 * The distinction is load-bearing, not cosmetic. `UrlSource` against a `blob:`
 * URL is browser-dependent and can degrade to fetching the whole file into
 * memory; a 500MB upload then sits in RAM.
 *
 * `blob` is only the right choice for a DISK-BACKED `File` (drag-drop, file
 * input), where `slice()` reads lazily. A Blob you materialized yourself from
 * bytes pins all of them — that cost ~600MB per 4K session before the filmstrip
 * worker moved to `UrlSource`. Desktop reads through the asset protocol and
 * stays on `url`.
 *
 * Kept free of `mediabunny` imports so the barrel can re-export it — the
 * ref → `Source` construction lives in `mediabunny.ts` instead.
 */

export type MediaRef = { kind: "url"; url: string } | { kind: "blob"; blob: Blob };

/** Accept a bare URL or Blob at API boundaries without forcing every existing
 *  call site to wrap. */
export function toMediaRef(src: MediaRef | Blob | string): MediaRef {
	if (typeof src === "string") return { kind: "url", url: src };
	return src instanceof Blob ? { kind: "blob", blob: src } : src;
}

/** Extension of the last path segment, lowercased and without the dot. Empty
 *  when there is nothing extension-shaped to read — `blob:` URLs and unnamed
 *  Blobs both land here. */
function extensionOf(name: string): string {
	const last = name.split("/").pop() ?? "";
	const dot = last.lastIndexOf(".");
	if (dot <= 0) return "";
	const ext = last.slice(dot + 1).toLowerCase();
	return /^[a-z0-9]{1,5}$/.test(ext) ? ext : "";
}

/** Container extension for the unsupported-format guard. Falls back to the
 *  Blob's MIME subtype, which is all a pasted/dropped Blob carries. */
export function mediaRefExtension(ref: MediaRef): string {
	if (ref.kind === "url") return extensionOf(ref.url.split(/[?#]/)[0] ?? "");
	const named = extensionOf((ref.blob as File).name ?? "");
	if (named) return named;
	const subtype = ref.blob.type.split("/")[1]?.split(";")[0]?.toLowerCase() ?? "";
	return /^[a-z0-9]{1,5}$/.test(subtype) ? subtype : "";
}

/** Stable identity for cache scoping. Derived from name+size+mtime rather than
 *  an object URL so re-opening the same file reuses its cached frames. */
export function mediaRefKey(ref: MediaRef): string {
	if (ref.kind === "url") return ref.url;
	const file = ref.blob as File;
	return `blob:${file.name ?? ""}:${ref.blob.size}:${file.lastModified ?? 0}`;
}
