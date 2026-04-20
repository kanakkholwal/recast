/**
 * Build WebP thumbnails for every wallpaper in static/wallpapers/*.{png,jpg,jpeg}
 *
 * Why: the full-res PNGs total ~157 MB. Loading them all in the background picker
 * is brutal on memory and first paint. We ship ~200 KB thumbs instead and only load
 * the full-res asset when a wallpaper is applied to the canvas.
 *
 * Run: bun run wallpapers:build
 * Outputs: static/wallpapers/thumbs/<name>.webp
 */

import { existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(SCRIPTS_DIR, "..");
const WALLPAPERS_DIR = join(ROOT, "assets/backgrounds/wallpapers");
const THUMBS_DIR = join(ROOT, "assets/backgrounds/thumbs");

const THUMB_WIDTH = 320; // displayed at ~80–110 px in a 3-col grid, so 2–3x DPR
const THUMB_QUALITY = 78;

const SOURCE_EXTS = new Set([".png", ".jpg", ".jpeg"]);

function bytes(n) {
	if (n < 1024) return `${n} B`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
	return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

async function main() {
	if (!existsSync(WALLPAPERS_DIR)) {
		console.error(`wallpapers dir not found: ${WALLPAPERS_DIR}`);
		process.exit(1);
	}
	if (!existsSync(THUMBS_DIR)) {
		mkdirSync(THUMBS_DIR, { recursive: true });
	}

	const files = readdirSync(WALLPAPERS_DIR)
		.filter((f) => SOURCE_EXTS.has(extname(f).toLowerCase()))
		.sort();

	if (files.length === 0) {
		console.log("No wallpapers found. Nothing to do.");
		return;
	}

	let totalSrc = 0;
	let totalThumb = 0;
	const results = [];

	for (const file of files) {
		const srcPath = join(WALLPAPERS_DIR, file);
		const name = basename(file, extname(file));
		const outPath = join(THUMBS_DIR, `${name}.webp`);

		const srcStat = statSync(srcPath);
		totalSrc += srcStat.size;

		await sharp(srcPath)
			.resize({ width: THUMB_WIDTH, withoutEnlargement: true, fit: "inside" })
			.webp({ quality: THUMB_QUALITY, effort: 5 })
			.toFile(outPath);

		const thumbStat = statSync(outPath);
		totalThumb += thumbStat.size;
		results.push({ file, src: srcStat.size, thumb: thumbStat.size });
	}

	console.log("");
	console.log("Generated wallpaper thumbnails");
	console.log("".padEnd(56, "─"));
	for (const r of results) {
		console.log(
			`  ${r.file.padEnd(22)}  ${bytes(r.src).padStart(10)}  →  ${bytes(r.thumb).padStart(9)}`,
		);
	}
	console.log("".padEnd(56, "─"));
	console.log(
		`  Total                   ${bytes(totalSrc).padStart(10)}  →  ${bytes(totalThumb).padStart(9)}`,
	);
	const ratio = ((totalThumb / totalSrc) * 100).toFixed(1);
	console.log(`  Size ratio              ${ratio.padStart(10)}%`);
	console.log("");
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
