import { browser } from "$app/environment";

let tauri: boolean | null = null;

export async function isTauriApp(): Promise<boolean> {
	if (!browser) return false;
	if (tauri !== null) return tauri;

	try {
		const mod = await import("@tauri-apps/api/core");
		tauri = mod.isTauri();
	} catch {
		tauri = false;
	}

	return tauri;
}
export async function getTauriTheme() {
	if (!browser) return null;
	const isTauri = await isTauriApp();
	if (!isTauri) return null;
	try {
		const { getCurrentWindow } = await import("@tauri-apps/api/window");
		const theme = await getCurrentWindow().theme();
		return theme;
	} catch (e) {
		console.error(e);
		return null;
	}
}
export async function listenToTauriTheme(cb: (theme: string) => void) {
	if (!browser) return () => {};
	const isTauri = await isTauriApp();
	if (!isTauri) return () => {};

	try {
		const { getCurrentWindow } = await import("@tauri-apps/api/window");
		const unlisten = await getCurrentWindow().onThemeChanged(({ payload: theme }) => {
			cb(theme);
		});
		return unlisten;
	} catch (e) {
		console.error(e);
		return () => {};
	}
}
