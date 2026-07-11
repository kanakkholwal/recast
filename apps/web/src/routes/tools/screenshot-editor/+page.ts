// The editor is a client-only app (canvas snapshot, clipboard, File APIs), so
// skip SSR/prerender for this route and mount it in the browser.
export const ssr = false;
export const prerender = false;
