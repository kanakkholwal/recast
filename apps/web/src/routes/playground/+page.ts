// Prerendered marketing + drop surface; the editor itself is the ssr=false child.
export const prerender = true;

export function load() {
	return { customSeo: true };
}
