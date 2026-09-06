// Every page here renders its own <SeoMeta>, so suppress the root layout's defaults to avoid duplicate tags.
export const load = () => ({ customSeo: true });
