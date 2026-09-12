import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/** The scheme `asset_scheme.rs` serves. Confined to app roots, opened projects, document-named and picked files. */
export const ASSET_SCHEME = "recast-asset";

/** A webview URL for an absolute path. Refused with 403 unless the core has granted the path. */
export function fileUrl(path: string): string {
	return convertFileSrc(path, ASSET_SCHEME);
}

/** Makes a file the user just picked readable through `fileUrl`. */
export function grantAssetPath(path: string): Promise<void> {
	return invoke<void>("grant_asset_path", { path });
}
