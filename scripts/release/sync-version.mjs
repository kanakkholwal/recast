#!/usr/bin/env node
// Rewrite the build manifests to the release version derived from a git tag.
//
// Source files keep "0.0.0-0" as a placeholder so local `tauri build`
// artifacts are obviously dev-stamped; the "-0" numeric pre-release is
// required by the Windows MSI bundler, which rejects non-numeric or
// >65535 pre-release identifiers. Releases are cut by tag, and this
// script is the single source of truth that rewrites the placeholder
// into the real version across:
//
//   - apps/desktop/src-tauri/tauri.conf.json   (drives bundle filenames)
//   - apps/desktop/package.json                (workspace version)
//   - apps/desktop/src-tauri/Cargo.toml        ([package].version line)
//
// Usage:
//   TAG=v1.2.3 node scripts/release/sync-version.mjs
//   node scripts/release/sync-version.mjs --tag v1.2.3
//
// Refuses to run if the tag resolves to the dev placeholder ("0.0.0-0")
// or an empty string.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPT_DIR, "..", "..");

const args = process.argv.slice(2);
const tagFromArg = (() => {
  const i = args.indexOf("--tag");
  return i >= 0 ? args[i + 1] : undefined;
})();
const tag = tagFromArg ?? process.env.TAG;

if (!tag) {
  console.error("::error::sync-version: no tag provided (set TAG env var or pass --tag)");
  process.exit(1);
}

const version = tag.startsWith("v") ? tag.slice(1) : tag;

if (!version) {
  console.error(`::error::sync-version: resolved empty version from tag '${tag}'`);
  process.exit(1);
}
if (version === "0.0.0-0") {
  console.error(
    `::error::sync-version: refusing to release with the dev placeholder. ` +
      `Tag '${tag}' resolved to '${version}'.`
  );
  process.exit(1);
}

console.log(`Syncing build manifests to version ${version}`);

const tauriConf = resolve(REPO_ROOT, "apps/desktop/src-tauri/tauri.conf.json");
const cargoToml = resolve(REPO_ROOT, "apps/desktop/src-tauri/Cargo.toml");
const pkgJson = resolve(REPO_ROOT, "apps/desktop/package.json");

// tauri.conf.json — drives the bundled artifact filenames
// (e.g. recast_${version}_x64_en-US.msi).
{
  const json = JSON.parse(readFileSync(tauriConf, "utf8"));
  json.version = version;
  writeFileSync(tauriConf, JSON.stringify(json, null, 2) + "\n");
  console.log(`  tauri.conf.json: ${json.version}`);
}

// package.json — keep the workspace version aligned with the release.
{
  const json = JSON.parse(readFileSync(pkgJson, "utf8"));
  json.version = version;
  writeFileSync(pkgJson, JSON.stringify(json, null, 2) + "\n");
  console.log(`  package.json:    ${json.version}`);
}

// Cargo.toml — only the [package] version line, leave dep versions alone.
// The [package] section is the first table in the file and the first
// `version = "..."` line we encounter belongs to it. Replace exactly
// once; subsequent matches (in dep versions) are left untouched.
{
  const lines = readFileSync(cargoToml, "utf8").split(/\r?\n/);
  const versionLineRe = /^version\s*=\s*"[^"]*"\s*$/;
  let replaced = false;
  for (let i = 0; i < lines.length; i++) {
    if (!replaced && versionLineRe.test(lines[i])) {
      lines[i] = `version = "${version}"`;
      replaced = true;
    }
  }
  if (!replaced) {
    console.error(
      `::error::sync-version: no [package].version line found in Cargo.toml`
    );
    process.exit(1);
  }
  writeFileSync(cargoToml, lines.join("\n"));
  console.log(`  Cargo.toml:      ${version}`);
}
