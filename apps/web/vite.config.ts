import { existsSync } from "node:fs";
import { docvia } from "@docvia/plugin-vite";
import adapter_auto from "@sveltejs/adapter-auto";
import adapter_cf from "@sveltejs/adapter-cloudflare";

import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import docviaConfig from "./docvia.config.ts";

// Cloudflare only when wrangler.jsonc is present; otherwise adapter-auto for Vercel/Node/preview.
const useCloudflare = existsSync(new URL("./wrangler.jsonc", import.meta.url));

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes("node_modules") ? undefined : true,
			},

			// Cloudflare Workers: the build lands in .svelte-kit/cloudflare and wrangler.jsonc points the deploy at it.
			adapter: useCloudflare ? adapter_cf() : adapter_auto(),
			prerender: {
				// Without this, prerendered pages bake SvelteKit's placeholder origin into canonical and og:url.
				origin: process.env.PUBLIC_APP_URL ?? "https://recast.li",

				// Still fails the build; this only adds the crawl context `handleError` can't see, such as which page linked to the failure.
				handleHttpError: ({ status, path, referrer, referenceType, message }) => {
					console.error(
						`[prerender] ${status} ${path} (${referenceType} from ${referrer ?? "entry point"}) :: ${message}`,
					);
					throw new Error(`Prerender failed: ${status} ${path}`);
				},
			},

			alias: {
				$components: "src/components",
				$utils: "src/utils",
				$hooks: "src/lib/hooks",
				$constants: "src/constants",
				$tools: "src/tools",
				$stores: "src/stores",
			},
		}),
		docvia(docviaConfig),
	],
	clearScreen: false,
	// Externalised SSR deps skip Vite transforms, so Node gets a raw `?url` specifier and crashes; bundling lets Vite inline the wasm.
	ssr: {
		noExternal: ["@takumi-rs/wasm"],
	},
	build: {
		// Inline the takumi wasm: Vercel's function can't read the static assets dir, and 5 MB is too large for an Edge function.
		assetsInlineLimit: (filePath) => (filePath.includes("takumi_wasm_bg") ? true : undefined),
	},
	// Surfaced as a global so analytics can tag every event with the running build.
	define: {
		__APP_VERSION__: JSON.stringify(process.env.npm_package_version ?? "0.0.0"),
	},
	server: {
		port: 4420,
		strictPort: true,
		host: "0.0.0.0",
		watch: {
			// tell vite to ignore watching `src-tauri`
			ignored: ["**/src-tauri/**"],
		},
		// Warm the highest-traffic routes so the first nav skips the cold-compile tax; a long list only adds worker-pool pressure.
		warmup: {
			clientFiles: [
				"./src/routes/+layout.svelte",
				"./src/routes/+page.svelte",
				"./src/routes/dashboard/+layout.svelte",
				"./src/routes/dashboard/+page.svelte",
				"./src/routes/share/[id]/+page.svelte",
				"./src/routes/(auth)/login/+page.svelte",
				"./src/lib/auth/client.ts",
			],
		},
	},
	// Pre-bundle heavy deps so the first navigation doesn't hit Vite's discovery path and force a full client reload.
	optimizeDeps: {
		include: [
			"@recast/icons",
			"better-auth/client/plugins",
			"better-auth/svelte",
			"bits-ui",
			"clsx",
			"mode-watcher",
			"svelte-sonner",
			"tailwind-merge",
			"tailwind-variants",
			// A transitive dep of @recast/analytics: pre-bundle it so the first capture doesn't trigger a reload cascade.
			"posthog-js",
		],
		exclude: [
			// Workspace packages stay unbundled so edits to packages/* hot-reload instead of being re-optimized as external deps.
			"@recast/ui",
			"@recast/design",
			"@recast/icons",
			"@recast/player",
			"@recast/analytics",
			"@recast/captions",
			"@recast/media",
			"@recast/editor",
		],
	},
	// Env variables starting with the item of `envPrefix` will be exposed in tauri's source code through `import.meta.env`.
	envPrefix: ["VITE_", "TAURI_ENV_*"],
});
