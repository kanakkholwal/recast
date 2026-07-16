import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import pkg from './package.json' with { type: 'json' };



export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit()
	],
	define: {
		__NAME__: `"${pkg.name}"`,
		__VERSION__: `"${pkg.version}"`,
	},
	clearScreen: false,
	server: {
		port: 4421,
		strictPort: true,
		host: "0.0.0.0",
		watch: {
			// tell vite to ignore watching `src-tauri`
			ignored: ['**/src-tauri/**'],
		},

	},
	// The `@recast/*` packages ship SOURCE (their exports map points at ./src),
	// so this app compiles their `.svelte`/`.ts` files itself. That means it must
	// resolve the leaf libs those files statically import (bits-ui, clsx, etc.).
	// Under pnpm those libs live in each package's own node_modules, not this
	// app's root, so vite can't resolve them from here unless they're real deps
	// of this app — hence they're declared in package.json AND pre-bundled here
	// (which also avoids first-request "new dep optimized, reloading" reloads).
	// Mirrors apps/web's working config.
	//
	// posthog-js is deliberately NOT here: it stays scoped to @recast/analytics
	// and is only ever reached via a dynamic `import("posthog-js")` once analytics
	// consent stands the provider up. Vite resolves that lazily at runtime from
	// the analytics package's own node_modules, so it's never pulled into this
	// app's bundle — listing it would just fail to pre-bundle (not a dep here)
	// and emit a spurious "failed to resolve" warning.
	optimizeDeps: {
		include: [
			'@recast/icons',
			'bits-ui',
			'clsx',
			'mode-watcher',
			'svelte-sonner',
			'tailwind-merge',
			'tailwind-variants',
		],
		exclude: [
			'@recast/ui',
			'@recast/design',
			'@recast/icons',
			'@recast/player',
			'@recast/analytics',
			'@recast/captions',
		],
	},
	// Env variables starting with the item of `envPrefix` are exposed to the
	// webview via `import.meta.env`. We use the `PUBLIC_` prefix (matching the
	// web app and SvelteKit's convention) rather than `VITE_` so the SAME var
	// name — e.g. `PUBLIC_POSTHOG_KEY` — can be consumed by BOTH the Svelte
	// frontend (here) and the Rust backend (which reads it prefix-agnostically
	// via std::env::var / option_env!). `TAURI_ENV_*` is Tauri's own injected
	// build context and must stay.
	envPrefix: ['PUBLIC_', 'TAURI_ENV_*']
});
