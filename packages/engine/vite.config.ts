import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		// The golden arm is a Playwright suite, not a vitest one: it needs a real
		// browser, because the browser build IS what it tests.
		exclude: ["**/node_modules/**", "test/golden/**"],
	},
});
