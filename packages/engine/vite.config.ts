import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		// The golden arm is a Playwright suite, not vitest: the browser build IS what it tests.
		exclude: ["**/node_modules/**", "test/golden/**"],
	},
});
