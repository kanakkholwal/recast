/**
 * Build WebP thumbnails + an external-asset manifest for every wallpaper in
 * `assets/backgrounds/wallpapers/*.{png,jpg,jpeg}`.
 *
 * Thumbnails land in `apps/desktop/static/backgrounds/thumbs/` (bundled with
 * the app — used in the picker and as offline placeholders). Full-res PNGs
 * stay at `assets/backgrounds/wallpapers/` and are NOT bundled — instead the
 * manifest (written to `assets/manifest.json`) drives runtime download into
 * the app's cache.
 *
 * Env: RELEASE_TAG (default `wallpapers-v1`), GH_REPO (default kanakkholwal/recast).
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(SCRIPTS_DIR, "..");
const WALLPAPERS_DIR = join(ROOT, "assets/backgrounds/wallpapers");
const THUMBS_DIR = join(ROOT, "apps/desktop/static/backgrounds/thumbs");
const MANIFEST_PATH = join(ROOT, "assets/manifest.json");

const RELEASE_TAG = process.env.RELEASE_TAG ?? "wallpapers-v1";
const GH_REPO = process.env.GH_REPO ?? "kanakkholwal/recast";

const THUMB_WIDTH = 320;
const THUMB_QUALITY = 78;

const SOURCE_EXTS = new Set([".png", ".jpg", ".jpeg"]);

function bytes(n) {
	if (n < 1024) return `${n} B`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
	return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function sha256File(path) {
	const hash = createHash("sha256");
	hash.update(readFileSync(path));
	return hash.digest("hex");
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
	const manifestAssets = [];

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

		const sha256 = sha256File(srcPath);
		manifestAssets.push({
			id: name,
			filename: file,
			url: `https://github.com/${GH_REPO}/releases/download/${RELEASE_TAG}/${file}`,
			sha256,
			size: srcStat.size,
		});

		results.push({ file, src: srcStat.size, thumb: thumbStat.size });
	}

	const manifest = { version: RELEASE_TAG, assets: manifestAssets };
	writeFileSync(MANIFEST_PATH, `${JSON.stringify(manifest, null, 2)}\n`);

	console.log("");
	console.log("Generated wallpaper thumbnails + manifest");
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
	console.log(`  Manifest                ${MANIFEST_PATH}`);
	console.log(`  Release tag             ${RELEASE_TAG} (${GH_REPO})`);
	console.log("");
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
