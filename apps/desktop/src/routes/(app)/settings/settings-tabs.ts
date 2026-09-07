/**
 * Settings tab vocabulary and its URL param.
 *
 * The tab lives in the URL so a reload keeps you where you were, and so the
 * four "…in Settings first" redirects can land on the tab they mean instead of
 * dropping you on General to hunt for it.
 */
export const SETTINGS_TABS = ["general", "recording", "cloud", "diagnostics", "advanced"] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number];

export const SETTINGS_TAB_PARAM = "tab";

export const DEFAULT_SETTINGS_TAB: SettingsTab = "general";

/** Validate a raw `?tab=` value, or null so the caller keeps its default. */
export function parseSettingsTab(raw: string | null | undefined): SettingsTab | null {
	if (!raw) return null;
	const value = raw.trim().toLowerCase();
	return SETTINGS_TABS.find((t) => t === value) ?? null;
}

/** Link to a settings tab, e.g. `settingsHref("cloud")` → `/settings?tab=cloud`. */
export function settingsHref(tab: SettingsTab): string {
	return `/settings?${SETTINGS_TAB_PARAM}=${tab}`;
}
