import { defineConfig, devices } from "@playwright/test";

const PORT = 5599;

/**
 * The golden arm needs a real browser, because the thing under test IS the
 * browser build. Chromium with WebGPU forced on: without the flags it falls back
 * to WebGL2 on many machines, which would silently test a different backend than
 * the one the app ships on.
 */
export default defineConfig({
	testDir: "./test/golden",
	// One worker: the fixtures share a GPU, and two headless Chromiums compositing at once makes a golden run flaky.
	workers: 1,
	fullyParallel: false,
	reporter: process.env.CI ? "list" : "line",
	timeout: 120_000,
	use: {
		baseURL: `http://localhost:${PORT}`,
		launchOptions: {
			args: [
				"--enable-unsafe-webgpu",
				"--enable-features=Vulkan",
				// Headless Chromium has no display and refuses a GPU adapter without this, so the harness never starts.
				"--use-angle=default",
				"--use-gl=angle",
				"--ignore-gpu-blocklist",
			],
		},
	},
	projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
	webServer: {
		command: `pnpm exec vite --port ${PORT} --strictPort`,
		url: `http://localhost:${PORT}/test/golden/`,
		reuseExistingServer: !process.env.CI,
		timeout: 60_000,
	},
});
