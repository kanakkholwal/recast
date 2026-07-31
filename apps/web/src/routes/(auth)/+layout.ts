// SSR is on: these pages are the first thing a signed-out visitor sees, and the
// signed-in guard in +layout.server.ts has to redirect *before* paint. With
// `ssr = false` SvelteKit returns an empty shell without running loads, so the
// guard only fired after hydration — the login form flashed on every visit.
//
// No `load` here on purpose: a universal load's return value *replaces* the
// server load's data for this layout level, which would drop `socialProviders`.
export const prerender = false;
