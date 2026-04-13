import adapter from '@sveltejs/adapter-netlify';
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
		adapter: adapter({
			// if true, will create a Netlify Edge Function rather
			// than using standard Node-based functions
			edge: false,

			// if true, will split your app into multiple functions
			// instead of creating a single one for the entire app.
			// if `edge` is true, this option cannot be used
			split: false
		}),
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
