/**
 * Turn the source packs under `extensions/packs/*` into the **installable**
 * artifacts the desktop app consumes, and a curated **index** for the in-app
 * browse gallery. Mirrors `scripts/prepare-backgrounds.mjs`: compute SHA-256s,
 * template download URLs against a GitHub release tag, emit JSON.
 *
 * For each pack it emits an installable manifest (the source `assets[].file`
 * becomes `{ filename, url, sha256 }` matching the Rust `AssetEntry`), plus a
 * flattened copy of every asset for upload. `contributes` passes through
 * unchanged (the frontend interprets it).
 *
 * Outputs (all flat under extensions/dist/, mirroring a GitHub release's flat
 * asset layout so the same files serve locally and upload as one glob):
 *   <packId>__<filename>     every asset, flat-named
 *   <packId>.extension.json  installable manifest (what install_extension fetches)
 *   index.json               { version, extensions: [{ id, manifestUrl, … }] }
 *
 * Usable two ways:
 *   - CLI:    `node build-registry.mjs` (one-shot; prints the publish command).
 *   - Import: `buildRegistry({ baseUrl })` — used by `serve-extensions.mjs` to
 *             rebuild in-process on file changes.
 *
 * Env (CLI):
 *   RELEASE_TAG   GitHub release tag to template URLs against (default extensions-v1)
 *   GH_REPO       owner/repo for the release (default kanakkholwal/recast)
 *   BASE_URL      Override the URL base entirely — point at a local static server
 *                 for a no-network install dry-run.
 *
 * Publish flow:
 *   RELEASE_TAG=extensions-v1 node extensions/scripts/build-registry.mjs
 *   gh release create extensions-v1 extensions/dist/*
 */

import { createHash } from "node:crypto";
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	rmSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
export const EXT_ROOT = resolve(SCRIPTS_DIR, "..");
export const PACKS_DIR = join(EXT_ROOT, "packs");
export const DIST_DIR = join(EXT_ROOT, "dist");

const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

function packDirs() {
	if (!existsSync(PACKS_DIR)) return [];
	return readdirSync(PACKS_DIR)
		.map((n) => join(PACKS_DIR, n))
		.filter((p) => statSync(p).isDirectory());
}

function buildPack(dir, base) {
	const src = JSON.parse(readFileSync(join(dir, "extension.json"), "utf8"));
	const packId = src.id;

	const assets = src.assets.map((a) => {
		// Guard against a malicious/edited pack pointing `file` outside its own
		// dir (e.g. `../../secrets`) — CI verifies packs, but this also protects
		// maintainers running the build script directly. Mirror the schema's
		// `^assets/<name>$` shape, then confirm the resolved path stays inside.
		if (typeof a.file !== "string" || !/^assets\/[^/\\]+$/.test(a.file)) {
			throw new Error(
				`${packId}: asset "${a.id}" has invalid file "${a.file}" (must be assets/<filename>)`,
			);
		}
		const abs = resolve(dir, a.file);
		const root = resolve(dir) + sep;
		if (!abs.startsWith(root)) {
			throw new Error(`${packId}: asset "${a.id}" file "${a.file}" escapes the pack directory`);
		}
		if (!existsSync(abs)) {
			throw new Error(`${packId}: asset "${a.id}" file missing at ${a.file}`);
		}
		const buf = readFileSync(abs);
		const filename = basename(a.file);
		const releaseName = `${packId}__${filename}`;
		copyFileSync(abs, join(DIST_DIR, releaseName));
		return {
			id: a.id,
			filename,
			url: `${base}/${releaseName}`,
			sha256: sha256(buf),
			size: buf.length,
		};
	});

	// Installable manifest — the exact envelope `install_extension` validates.
	const manifest = {
		id: packId,
		name: src.name,
		version: src.version,
		author: src.author ?? null,
		kind: src.kind,
		permissions: src.permissions ?? [],
		contributes: src.contributes ?? {},
		assets,
	};
	writeFileSync(
		join(DIST_DIR, `${packId}.extension.json`),
		`${JSON.stringify(manifest, null, 2)}\n`,
	);

	return {
		id: packId,
		name: src.name,
		version: src.version,
		author: src.author ?? undefined,
		description: src.description ?? undefined,
		manifestUrl: `${base}/${packId}.extension.json`,
	};
}

/**
 * Build all packs into `extensions/dist/`. Returns `{ dir, base, tag, entries }`.
 * `baseUrl` overrides the URL base (a local server during dev); otherwise it
 * templates against the GitHub release tag.
 */
export function buildRegistry({ baseUrl, tag } = {}) {
	const releaseTag = tag ?? process.env.RELEASE_TAG ?? "extensions-v1";
	const repo = process.env.GH_REPO ?? "kanakkholwal/recast";
	const base = (
		baseUrl ??
		process.env.BASE_URL ??
		`https://github.com/${repo}/releases/download/${releaseTag}`
	).replace(/\/+$/, "");

	rmSync(DIST_DIR, { recursive: true, force: true });
	mkdirSync(DIST_DIR, { recursive: true });

	const entries = packDirs()
		.map((dir) => buildPack(dir, base))
		.sort((a, b) => a.id.localeCompare(b.id));

	const index = { version: releaseTag, extensions: entries };
	writeFileSync(join(DIST_DIR, "index.json"), `${JSON.stringify(index, null, 2)}\n`);

	return { dir: DIST_DIR, base, tag: releaseTag, entries };
}

// - CLI --

const invokedDirectly = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href;

if (invokedDirectly) {
	const { dir, base, tag, entries } = buildRegistry();
	console.log(`Built ${entries.length} pack(s) -> ${dir}  (base: ${base})`);
	for (const e of entries) console.log(`  ${e.id}  ${e.manifestUrl}`);
	console.log(`\nPublish:\n  gh release create ${tag} extensions/dist/*`);
}
