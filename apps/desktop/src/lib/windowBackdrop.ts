/**
 * Translucent window backdrop (Win11 Mica, macOS vibrancy) for the app windows
 * (main, editor), gated by the `window_transparency` setting. The bespoke
 * transparent overlays (panel, pickers, region select) opt out via the root
 * layout. Unsupported platforms/GPUs fall back to solid automatically.
 */

import { Effect, getCurrentWindow } from "@tauri-apps/api/window";
import { platform } from "@tauri-apps/plugin-os";
import { getWindowTransparency } from "$lib/ipc";

/** Broadcast so every open window re-applies when the setting is toggled. */
export const BACKDROP_CHANGED_EVENT = "window-transparency-changed";

function effectsForOs(): Effect[] {
	const os = platform();
	if (os === "windows") return [Effect.Mica];
	if (os === "macos") return [Effect.UnderWindowBackground];
	return [];
}

/**
 * Apply or clear the backdrop for the current window. Reads the setting when
 * `enabled` is omitted. Toggles `window-transparent` on the root so CSS lets the
 * material show through the frame while content surfaces stay opaque.
 */
export async function applyWindowBackdrop(enabled?: boolean): Promise<void> {
	const win = getCurrentWindow();
	const root = document.documentElement;

	const on = enabled ?? (await getWindowTransparency().catch(() => false));
	const effects = on ? effectsForOs() : [];

	if (effects.length === 0) {
		root.classList.remove("window-transparent");
		try {
			await win.clearEffects();
		} catch {
			// No effect was set; ignore.
		}
		return;
	}

	try {
		await win.setEffects({ effects });
		root.classList.add("window-transparent");
	} catch (e) {
		console.warn("[backdrop] setEffects failed", e);
		root.classList.remove("window-transparent");
	}
}
