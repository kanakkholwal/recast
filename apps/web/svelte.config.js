import adapter from '@sveltejs/adapter-auto';
// import adapter from '@sveltejs/adapter-cloudflare';


/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		alias: {
			$components: 'src/components',
			$utils: 'src/utils',
			$hooks: 'src/lib/hooks',
			$constants: 'src/constants',
			$tools: 'src/tools',
			$stores: 'src/stores',
			"@": "./src/@",
		},
		adapter: adapter(),
		prerender: {
			// Without this, prerendered pages bake in SvelteKit's placeholder origin
			// (http://sveltekit-prerender) as their <link rel=canonical> and og:url,
			// which points crawlers at a domain that does not exist.
			origin: process.env.PUBLIC_APP_URL ?? "https://recast.li",
		},
		// cloudflare
			// adapter: adapter({
		// 	// See below for an explanation of these options
		// 	config: undefined,
		// 	platformProxy: {
		// 		configPath: undefined,
		// 		environment: undefined,
		// 		persist: undefined
		// 	},
		// 	fallback: 'plaintext',
		// 	routes: {
		// 		include: ['/*'],
		// 		exclude: ['<all>']
		// 	}
		// }),
	}
};

export default config;
