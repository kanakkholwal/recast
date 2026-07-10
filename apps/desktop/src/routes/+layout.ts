// Tauri SPA: SSR and prerender both OFF (AGENTS.md §4 "Desktop"). There's no
// server, and the dynamic `editor/[file]` route can't prerender anyway. The
// adapter-static `fallback` page serves every route as a client-side SPA.
export const prerender = false;
export const ssr = false;
