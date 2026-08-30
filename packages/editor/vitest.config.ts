import { defineConfig } from "vitest/config";

// Plain Node, no Svelte plugins: this suite targets pure modules, so runes modules must be imported type-only.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
