/**
 * Pure parser + builder for `recast://` deep links. Framework-free (no Tauri/
 * SvelteKit imports) so it runs in the node unit suite. Dispatch lives in
 * `deepLink.ts`. Parser and builder are inverses (round-trip tested); the
 * builder is the safe entry point for automation/MCP to emit links.
 *
 * Grammar:
 *   recast://open?path=<uri-encoded absolute path>   → open a project
 *   recast://go?to=<uri-encoded in-app route>        → navigate the main window
 */
export type DeepLinkAction =
  | { kind: "open-project"; path: string }
  | { kind: "navigate"; route: string };

/**
 * Top-level segments of the `(app)` route group that `go?to=` may target. Keep
 * in sync with `src/routes/(app)/`. `""` is the home route (`/`).
 */
const ALLOWED_ROUTES = new Set([
  "",
  "recasts",
  "exports",
  "profiles",
  "settings",
  "whats-new",
]);

/**
 * Whether a `go?to=` route is a safe in-app target: absolute, not protocol-
 * relative, no traversal, and its first segment is on the allowlist. Shared by
 * the parser and builder so they can never disagree.
 */
export function isAllowedRoute(route: string): boolean {
  if (!route.startsWith("/") || route.startsWith("//")) return false;
  if (route.includes("..")) return false;
  return ALLOWED_ROUTES.has(route.split("/")[1] ?? "");
}

/**
 * Parse a `recast://` URL into an action, or `null` for anything unrecognised
 * or unsafe (unknown host, wrong protocol, malformed, off-allowlist route,
 * protocol-relative, or traversal). Never throws.
 */
export function parseDeepLink(raw: string): DeepLinkAction | null {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return null;
  }

  if (url.protocol !== "recast:") return null;

  if (url.hostname === "open") {
    const path = url.searchParams.get("path");
    return path ? { kind: "open-project", path } : null;
  }

  if (url.hostname === "go") {
    const to = url.searchParams.get("to");
    if (!to || !isAllowedRoute(to)) return null;
    return { kind: "navigate", route: to };
  }

  return null;
}

/** Build a `recast://open?path=…` link for a project file. */
export function buildOpenProjectLink(path: string): string {
  return `recast://open?path=${encodeURIComponent(path)}`;
}

/**
 * Build a `recast://go?to=…` navigation link. Throws on a route the parser
 * would reject, so automation can't mint a dead link.
 */
export function buildNavigateLink(route: string): string {
  if (!isAllowedRoute(route)) {
    throw new Error(`Route not allowed for deep link: ${route}`);
  }
  return `recast://go?to=${encodeURIComponent(route)}`;
}

/** Inverse of `parseDeepLink`: turn an action back into a `recast://` URL. */
export function buildDeepLink(action: DeepLinkAction): string {
  return action.kind === "open-project"
    ? buildOpenProjectLink(action.path)
    : buildNavigateLink(action.route);
}
