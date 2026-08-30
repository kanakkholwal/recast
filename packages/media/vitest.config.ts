import { defineConfig } from "vitest/config";

// Pure-logic tests run under node for speed; browser fixtures opt in per file. `resolve.extensions` is listed so `test/` can import sibling sources extensionless.
export default defineConfig({
	resolve: {
		extensions: [".ts", ".tsx", ".js", ".mjs", ".mts"],
	},
	test: {
		include: ["src/**/*.{test,spec}.ts", "test/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
