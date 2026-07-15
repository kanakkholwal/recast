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
