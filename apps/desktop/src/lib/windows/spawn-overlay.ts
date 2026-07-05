/** Spawn-or-focus helper for singleton overlay webviews (device picker, etc.). */

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

type OverlayOptions = NonNullable<ConstructorParameters<typeof WebviewWindow>[1]>;

/**
 * Open an overlay webview labelled `label`, focusing an existing window with
 * that label instead of spawning a duplicate.
 */
export async function spawnOverlayWindow(
	label: string,
	options: OverlayOptions,
): Promise<void> {
	const existing = await WebviewWindow.getByLabel(label);
	if (existing) {
		await existing.setFocus();
		return;
	}
	new WebviewWindow(label, options);
}
