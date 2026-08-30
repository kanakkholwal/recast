import { defineConfig } from "vitest/config";

// Pure data and arithmetic, so a plain Node run keeps it fast; the Rust ASS generator mirrors these against the same fixture.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
