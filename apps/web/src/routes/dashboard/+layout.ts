// SSR is off because the pages lean on localStorage-backed stores; +layout.server.ts still runs the real session check.
export const ssr = false;
export const prerender = false;
