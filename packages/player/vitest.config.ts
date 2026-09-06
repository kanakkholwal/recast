import { defineConfig } from "vitest/config";

// Player logic is pure arithmetic and key/URL policy, so a plain Node run is enough; media elements are verified in-app.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
