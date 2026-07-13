import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Standalone vitest config — intentionally does NOT load the SvelteKit/Tailwind
// plugins from vite.config.ts. The unit suite targets pure, framework-free logic
// (the extracted `.logic.ts` modules), so a plain Node environment keeps the
// tests fast and free of browser/Svelte setup. Mirrors apps/desktop.
//
// The `$lib` alias mirrors SvelteKit's so extracted `.logic.ts` modules can use
// the same import specifier as the app. Only pure modules resolve here; runes
// (`.svelte.ts`) modules must be imported type-only to stay out of the Node run.
export default defineConfig({
	resolve: {
		alias: {
			$lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
		},
	},
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
