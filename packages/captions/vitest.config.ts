import { defineConfig } from "vitest/config";

// The caption model is pure data + arithmetic (chunking, highlight, line
// breaking, pill geometry, VTT). No Svelte, no DOM, so a plain Node run keeps
// it fast. The Rust ASS generator mirrors these same functions and is checked
// against the same fixture in src/__fixtures__/caption-parity.json.
export default defineConfig({
	test: {
		include: ["src/**/*.{test,spec}.ts"],
		environment: "node",
	},
});
