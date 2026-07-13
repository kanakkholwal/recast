// The editor is a client-only app (canvas snapshot, clipboard, File APIs), so
// skip SSR/prerender for this route and mount it in the browser. Its marketing
// landing at /tools/screenshot-editor IS prerendered, and that is the page
// search engines index.
export const ssr = false;
export const prerender = false;
