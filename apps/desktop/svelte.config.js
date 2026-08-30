import adapter from "@sveltejs/adapter-static";

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		// See https://svelte.dev/docs/kit/adapters for the adapter options.
		adapter: adapter({
			fallback: "index.html", // may differ from host to host
		}),
		alias: {
			$components: "src/components",
			$utils: "src/utils",
			$hooks: "src/lib/hooks",
			$constants: "src/constants",
			$tools: "src/tools",
			$stores: "src/stores",
			"@": "./src/@",
		},
	},
};

export default config;
