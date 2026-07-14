import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { docvia } from "@docvia/plugin-vite";
import docviaConfig from "./docvia.config";

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit(),
		docvia(docviaConfig)
	],
	clearScreen: false,
	// `@takumi-rs/wasm` (used by /api/og) ships its WebAssembly binary as an
	// asset. Externalised SSR deps skip Vite transforms, so Node would receive a
	// raw `?url` specifier and crash — bundling the package lets Vite resolve and
	// inline it (see assetsInlineLimit). The native `@takumi-rs/core` addon
	// doesn't bundle on Vercel, so /api/og runs the wasm renderer instead.
	ssr: {
		noExternal: ['@takumi-rs/wasm'],
	},
	build: {
		// Base64-inline the takumi wasm so its bytes ship *inside* the /api/og
		// server bundle. On Vercel the serverless function can't read the client/
		// static assets dir takumi's stock loader expects, and the 5 MB binary is
		// too large for an Edge function — inlining sidesteps both. Everything
		// else keeps Vite's default size threshold (return undefined).
		assetsInlineLimit: (filePath) =>
			filePath.includes('takumi_wasm_bg') ? true : undefined,
	},
	// Surfaced as a global so analytics can tag every event with the running
	// build. npm_package_version is set by the pnpm/npm script runner.
	define: {
		__APP_VERSION__: JSON.stringify(process.env.npm_package_version ?? "0.0.0"),
	},
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
			// posthog-js is a transitive dep of @recast/analytics (dynamic import);
			// pre-bundle it so the first capture doesn't trigger a reload cascade.
			'posthog-js',
		],
		exclude: [
			// Workspace packages — leave them out of prebundling so edits
			// to packages/* hot-reload instantly instead of getting
			// re-optimized as if they were external deps.
			'@recast/ui',
			'@recast/design',
			'@recast/player',
			'@recast/analytics',
			'@recast/captions',
		],
	},
	// Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
	envPrefix: ['VITE_', 'TAURI_ENV_*']
});
