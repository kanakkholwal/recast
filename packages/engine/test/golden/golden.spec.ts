/**
 * The WASM arm of the golden harness.
 *
 * The native arm (`crates/recast-compositor/tests/golden.rs`) proves the
 * compositor draws the right picture. This one proves the BROWSER build of that
 * same compositor draws it too, by rendering the same fixtures through
 * `@recast/engine` and comparing against the same committed PNGs. That
 * comparison is the gate the preview and export swap rests on: without it,
 * "one compositor" is a claim about the source tree rather than about pixels.
 *
 * Both arms read `crates/recast-compositor/tests/goldens/`: `fixtures.json` for
 * the scenes, `source.png` and `background.png` for the inputs. Neither arm owns
 * a private copy of either.
 */

import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, type Page, test } from "@playwright/test";
import sharp from "sharp";

const HERE = dirname(fileURLToPath(import.meta.url));
const GOLDENS = join(HERE, "../../../../crates/recast-compositor/tests/goldens");

import { frameDelta, isWithin } from "./delta";
import { GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN } from "./tolerance";

interface Fixture {
	name: string;
	time: number;
	overrides: Record<string, unknown>;
}

interface FixtureFile {
	source: { width: number; height: number };
	base: Record<string, unknown>;
	fixtures: Fixture[];
}

async function rgba(name: string): Promise<{ data: Uint8Array; width: number; height: number }> {
	const { data, info } = await sharp(join(GOLDENS, name))
		.ensureAlpha()
		.raw()
		.toBuffer({ resolveWithObject: true });
	return { data: new Uint8Array(data), width: info.width, height: info.height };
}

async function fixtures(): Promise<FixtureFile> {
	return JSON.parse(await readFile(join(GOLDENS, "fixtures.json"), "utf8"));
}

/**
 * Which stack a run happened on. The committed PNGs came off native wgpu, so a
 * browser that does not match the recorded one REPORTS its deltas rather than
 * failing a machine that is not wrong — the same policy the native arm uses.
 */
const ADAPTER_FILE = join(GOLDENS, "ADAPTER.wasm");

async function recordedAdapter(): Promise<string | null> {
	return readFile(ADAPTER_FILE, "utf8")
		.then((s) => s.trim())
		.catch(() => null);
}

async function ready(page: Page) {
	const errors: string[] = [];
	page.on("console", (m) => {
		if (m.type() === "error") errors.push(m.text());
	});
	page.on("pageerror", (e) => errors.push(String(e)));
	await page.goto("/test/golden/");
	await page
		.waitForFunction(() => document.documentElement.dataset.goldenReady === "1", null, {
			timeout: 30_000,
		})
		.catch(() => {
			throw new Error(
				`the harness never came up:\n  ${errors.join("\n  ") || "no console errors"}`,
			);
		});
	return errors;
}

test("every fixture matches its golden through the browser build", async ({ page }) => {
	await ready(page);

	const all = await fixtures();
	const source = await rgba("source.png");
	const background = await rgba("background.png");
	expect(
		[source.width, source.height],
		"source.png must be the size fixtures.json declares",
	).toEqual([all.source.width, all.source.height]);

	const here = `${await page.evaluate(() => window.__golden.backend())} / ${await page.evaluate(
		() => window.__golden.adapter(),
	)}`;
	const recorded = await recordedAdapter();
	const updating = process.env.UPDATE_GOLDENS === "1";
	const gates = updating || recorded === here;

	const drifted: string[] = [];
	for (const fixture of all.fixtures) {
		const scene = { ...all.base, ...fixture.overrides };
		const result = await page.evaluate((request) => window.__golden.render(request), {
			scene,
			outputTime: fixture.time,
			sourceWidth: all.source.width,
			sourceHeight: all.source.height,
			source: Array.from(source.data),
			background: Array.from(background.data),
		});
		expect(result.layersDrawn, `${fixture.name} drew nothing`).toBeGreaterThan(0);

		const golden = await rgba(`${fixture.name}.png`);
		expect(
			[result.width, result.height],
			`${fixture.name}: the browser build sized the canvas differently from the native one`,
		).toEqual([golden.width, golden.height]);

		const delta = frameDelta(golden.data, new Uint8Array(result.pixels));
		if (!isWithin(delta, GOLDEN_MAX_CHANNEL, GOLDEN_MAX_MEAN)) {
			drifted.push(
				`${fixture.name}: max ${delta.maxChannel} mean ${delta.meanChannel.toFixed(3)} over ${delta.differingPixels} differing px`,
			);
			// Next to its golden, so the two can be compared by eye. Numbers say a
			// frame drifted; only the picture says how. Gitignored.
			await sharp(Buffer.from(result.pixels), {
				raw: { width: result.width, height: result.height, channels: 4 },
			})
				.png()
				.toFile(join(GOLDENS, `${fixture.name}.actual.png`));
		}
	}

	if (updating) {
		await writeFile(ADAPTER_FILE, `${here}\n`);
		throw new Error(`recorded the wasm adapter as ${here}; re-run without UPDATE_GOLDENS`);
	}
	if (!gates) {
		// Loud, and never silent: a suite that opts out quietly reads exactly like
		// one that passed.
		console.warn(
			`the wasm goldens were measured on ${recorded ?? "an unrecorded stack"}, this is ${here}: reporting only.\n  ` +
				(drifted.length ? drifted.join("\n  ") : "every fixture still matched"),
		);
		return;
	}
	expect(drifted, `the browser build drifted from the native goldens on ${here}`).toEqual([]);
});

/**
 * The size the engine picks IS the geometry. A shared evaluator that disagreed
 * on canvas size between the two builds would fail the pixel test above with an
 * unreadable message, so this says it plainly.
 */
test("the browser build derives the same canvas geometry as the native one", async ({ page }) => {
	await ready(page);
	const all = await fixtures();
	const source = await rgba("source.png");
	const background = await rgba("background.png");

	for (const fixture of all.fixtures) {
		const golden = await rgba(`${fixture.name}.png`);
		const result = await page.evaluate((request) => window.__golden.render(request), {
			scene: { ...all.base, ...fixture.overrides },
			outputTime: fixture.time,
			sourceWidth: all.source.width,
			sourceHeight: all.source.height,
			source: Array.from(source.data),
			background: Array.from(background.data),
		});
		expect([result.width, result.height], fixture.name).toEqual([golden.width, golden.height]);
	}
});
