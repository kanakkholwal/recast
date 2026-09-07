import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Standalone on purpose: the unit suite targets pure logic, so a plain Node run stays fast. The `$lib` alias mirrors SvelteKit's, and runes modules must be imported type-only.
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
