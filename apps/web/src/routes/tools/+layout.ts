// The tool pages render their own <SeoMeta> and JSON-LD, so suppress the root layout's defaults.
export const prerender = true;
export const load = () => ({ customSeo: true });
