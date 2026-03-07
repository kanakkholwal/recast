import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';



export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit()
	],
	clearScreen: false,
	server: {
		port: 4000,
		// open: true,
		strictPort: true,
		host: "0.0.0.0",
		watch: {
			// tell vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},

	},
	// Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
	envPrefix: ['VITE_', 'TAURI_ENV_*']
});
