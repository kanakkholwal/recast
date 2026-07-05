/**
 * Transparent overlay windows (camera preview, pickers, region select, panel).
 * These are too small to host Sonner/global chrome and own their own key
 * handling, so the root layout skips analytics/toast/shortcut bridges and page
 * transitions for them. Single source for both the boot gate and onNavigate.
 */

export const TRANSPARENT_ROUTES = [
	"/camera-preview",
	"/device-picker",
	"/profile-picker",
	"/select",
	"/panel",
];

/** True when `path` belongs to a transparent overlay window. */
export function isOverlayRoute(path: string): boolean {
	return TRANSPARENT_ROUTES.some((p) => path.startsWith(p));
}
