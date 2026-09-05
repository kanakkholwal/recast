import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, searchForWorkspaceRoot } from "vite";
import pkg from "./package.json" with { type: "json" };
import adapter from "@sveltejs/adapter-static";

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			adapter: adapter(),
			alias: {
				$components: "src/components",
				$utils: "src/utils",
				$hooks: "src/lib/hooks",
				$constants: "src/constants",
				$tools: "src/tools",
				$stores: "src/stores",
				"@": "./src/@",
			},
		}),
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
			ignored: ["**/src-tauri/**"],
		},
		fs: {
			// @recast/editor spawns workers from the sibling package's source, so the workspace root must be servable.
			allow: [searchForWorkspaceRoot(process.cwd())],
		},
	},

	optimizeDeps: {
		include: [
			"bits-ui",
			"clsx",
			"mode-watcher",
			"svelte-sonner",
			"tailwind-merge",
			"tailwind-variants",
		],
		exclude: [
			"@recast/ui",
			"@recast/design",
			"@recast/icons",
			"@recast/player",
			"@recast/analytics",
			"@recast/captions",
			// Ship SOURCE and spawn workers via `new URL(..., import.meta.url)`;
			"@recast/editor",
			"@recast/media",
		],
	},
	envPrefix: ["PUBLIC_", "TAURI_ENV_*"],
});
