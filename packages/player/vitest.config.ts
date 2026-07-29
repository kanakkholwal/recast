import { defineConfig } from "vitest/config";

// Player logic is pure arithmetic + key/URL policy, so a plain Node run is
// enough. Anything needing a real media element is verified in-app, not here.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
