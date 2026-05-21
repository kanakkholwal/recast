// Dashboard is auth-gated, not dev-gated — see +layout.server.ts for the
// real session check. We disable SSR because the page leans heavily on
// localStorage-backed stores; +layout.server.ts still runs to fetch the
// session and provide it to the client.
export const ssr = false;
export const prerender = false;
