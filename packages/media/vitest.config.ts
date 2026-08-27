import { defineConfig } from "vitest/config";

// The @recast/media package owns the non-linear editor's media-pipeline
// contract (see packages/media/REQUIREMENTS.md). Tests target pure logic
// (time maps, cache eviction, byte budgets) plus end-to-end perf-budget
// assertions against the published budgets table. The pure-logic tests stay
// under node for speed; the end-to-end fixtures (PR-D onwards) need a browser
// environment, so they'll switch to happy-dom via per-file `// @vitest-
// environment` comments.
//
// `resolve.extensions` mirrors Vite's defaults; listed explicitly so test
// files (which live under `test/`, not `src/`) can import sibling source
// modules extensionless and vitest resolves them to .ts.
export default defineConfig({
	resolve: {
		extensions: [".ts", ".tsx", ".js", ".mjs", ".mts"],
	},
	test: {
		include: ["src/**/*.{test,spec}.ts", "test/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
