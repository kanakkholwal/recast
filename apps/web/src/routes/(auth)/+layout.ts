import type { LayoutLoad } from "./$types";

export const ssr = false;
export const prerender = false;

// The dev-only 404 gate is gone — auth is live in production too. Per-route
// gating (signup → /waitlist) lives in each route's +page.ts.
export const load: LayoutLoad = () => ({});
