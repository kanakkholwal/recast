import { defineConfig } from "vitest/config";

// Plain Node run, no Svelte/SvelteKit plugins: this suite targets the pure
// modules (timeline math, time mapping, export scene building). Runes modules
// must be imported type-only so they stay out of it.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
