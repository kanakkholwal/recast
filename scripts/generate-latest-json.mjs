#!/usr/bin/env node
/**
 * Assemble the Tauri updater manifest (`latest.json`) from the signed bundle
 * signatures produced across the release matrix.
 *
 * The desktop release workflow builds each platform separately so it can keep
 * its custom MSIX packaging, FFmpeg sidecar downloads, and artifact
 * attestations — it does not use `tauri-action`, which is what would normally
 * emit this manifest. So we build it here from the `.sig` files instead.
 *
 * Each `.sig` is named `<bundle><ext>.sig`; the bundle it signs (which is also
 * the release asset the updater downloads) is just the filename with `.sig`
 * stripped. We only need the signatures — the manifest references the bundles
 * by URL on the GitHub release, it does not embed them.
 *
 * Usage:
 *   node scripts/generate-latest-json.mjs \
 *     --tag v1.2.3 --repo owner/name --dir <sig-dir> --out latest.json
 */
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

function arg(name, fallback) {
	const i = process.argv.indexOf(`--${name}`);
	const v = i !== -1 ? process.argv[i + 1] : undefined;
	return v && !v.startsWith("--") ? v : fallback;
}

const tag = arg("tag");
const repo = arg("repo");
const dir = arg("dir", ".");
const out = arg("out", "latest.json");

if (!tag || !repo) {
	console.error(
		"Usage: generate-latest-json.mjs --tag <tag> --repo <owner/name> --dir <dir> --out <file>",
	);
	process.exit(1);
}

const version = tag.replace(/^v/, "");
const sigs = readdirSync(dir).filter((f) => f.endsWith(".sig"));

const pick = (predicate) => sigs.find(predicate);

// Map each updater platform key to the signature of the bundle it installs.
// Windows updates via the NSIS installer (`-setup.exe`); macOS via the
// arch-tagged `.app.tar.gz`; Linux via the AppImage.
const targets = [
	{
		key: "windows-x86_64",
		sig:
			pick((f) => f.endsWith("-setup.exe.sig")) ??
			pick((f) => f.endsWith(".msi.sig")),
	},
	{
		key: "linux-x86_64",
		sig: pick((f) => f.endsWith(".AppImage.sig")),
	},
	{
		key: "darwin-aarch64",
		sig: pick(
			(f) => f.endsWith(".app.tar.gz.sig") && /aarch64|arm64/.test(f),
		),
	},
	{
		key: "darwin-x86_64",
		sig: pick(
			(f) =>
				f.endsWith(".app.tar.gz.sig") && /(?:^|[_-])x(?:64|86_64)/.test(f),
		),
	},
];

const platforms = {};
for (const { key, sig } of targets) {
	if (!sig) {
		console.warn(`::warning::No updater signature found for ${key} — skipping.`);
		continue;
	}
	const bundle = sig.slice(0, -".sig".length);
	platforms[key] = {
		signature: readFileSync(join(dir, sig), "utf8").trim(),
		url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(bundle)}`,
	};
}

if (Object.keys(platforms).length === 0) {
	console.error(
		"::error::No signed updater bundles found — refusing to write an empty " +
			"latest.json. Are the TAURI_SIGNING_PRIVATE_KEY secrets configured?",
	);
	process.exit(1);
}

const manifest = {
	version,
	notes: `See the full release notes at https://github.com/${repo}/releases/tag/${tag}`,
	pub_date: new Date().toISOString(),
	platforms,
};

writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(
	`Wrote ${out} for ${tag} — platforms: ${Object.keys(platforms).join(", ")}`,
);
