import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, searchForWorkspaceRoot } from "vite";
import pkg from "./package.json" with { type: "json" };

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
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
			// @recast/editor spawns workers via `new URL(..., import.meta.url)`, served
			// as direct file requests from the sibling package's source — allow the
			// workspace root so they resolve ("outside serving allow list" otherwise).
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
