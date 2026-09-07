export const prerender = false;
export const ssr = false;

import type { PageLoad } from "./$types";

export const load: PageLoad = ({ params }) => {
	const encoded = decodeURIComponent(params.file);
	const filePath = decodeURIComponent(atob(encoded));
	const filename = filePath.split(/[\\/]/).pop() || "Recording";

	return {
		filePath,
		filename,
	};
};
