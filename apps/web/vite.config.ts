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
		port: 4420,
		strictPort: true,
		host: "0.0.0.0",
		watch: {
			// tell vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},
		// Warm up the routes / leaf files that get hit on practically every
		// dev session. Vite kicks off transforms in parallel at boot so the
		// first nav doesn't pay the cold-compile tax. Keep this list small
		// and high-traffic — adding everything actually slows things down
		// (parallel pressure on the worker pool).
		warmup: {
			clientFiles: [
				'./src/routes/+layout.svelte',
				'./src/routes/+page.svelte',
				'./src/routes/dashboard/+layout.svelte',
				'./src/routes/dashboard/+page.svelte',
				'./src/routes/share/[id]/+page.svelte',
				'./src/routes/(auth)/login/+page.svelte',
				'./src/lib/auth/client.ts',
			],
		},
	},
	// Pre-bundle the heavy / always-used deps so first request doesn't
	// trigger a "new dep optimized, reloading" cascade. Without this, the
	// first navigation in dev mode hits Vite's discovery path and forces
	// a full client reload once new deps are found — extra noticeable on
	// the share page (player + bits-ui) and dashboard (drizzle/auth client).
	optimizeDeps: {
		include: [
			'@lucide/svelte',
			'better-auth/client/plugins',
			'better-auth/svelte',
			'bits-ui',
			'clsx',
			'mode-watcher',
			'svelte-sonner',
			'tailwind-merge',
			'tailwind-variants',
		],
		exclude: [
			// Workspace packages — leave them out of prebundling so edits
			// to packages/* hot-reload instantly instead of getting
			// re-optimized as if they were external deps.
			'@recast/ui',
			'@recast/design',
			'@recast/player',
		],
	},
	// Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
	envPrefix: ['VITE_', 'TAURI_ENV_*']
});
