// Server-rendered (not prerendered): the page reads `?email=`/`?source=` at
// render time, which prerendering forbids. Explicit `false` also tells the
// prerender crawler (which reaches this via /tools) to skip it. Renders its own
// <SeoMeta>, so suppress the root layout's default tags.
export const prerender = false;
export const load = () => ({ customSeo: true });
