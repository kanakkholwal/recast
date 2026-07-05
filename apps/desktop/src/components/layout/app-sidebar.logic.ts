/** Pure nav helpers for app-sidebar. */

/** Prefix-match a nav href against the current path; "/" matches only itself. */
export function isActive(path: string, currentPath: string): boolean {
	if (path === "/") return currentPath === "/";
	return currentPath.startsWith(path);
}
