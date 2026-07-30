/** Pure nav helpers for app-sidebar. */

/**
 * Match a nav href against the current path. "/" matches only itself, and a
 * prefix only counts at a segment boundary — a bare `startsWith` lit up
 * "Exports" for a hypothetical /exports-archive route.
 */
export function isActive(path: string, currentPath: string): boolean {
	if (path === "/") return currentPath === "/";
	return currentPath === path || currentPath.startsWith(`${path}/`);
}
