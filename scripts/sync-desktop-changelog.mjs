#!/usr/bin/env node
// Regenerate `apps/desktop/src/constants/changelog.ts` from `CHANGELOG.md`.
//
// CHANGELOG.md is the canonical Keep-a-Changelog source (also used by the
// release workflow via `scripts/extract-changelog.mjs`). The desktop app's
// "What's new" dialog and full changelog page read from a typed RELEASES
// array — this script parses CHANGELOG.md and rewrites that array between
// the `RELEASES:START` … `RELEASES:END` markers.
//
// Run manually with `pnpm sync:changelog`, or automatically before each
// desktop build (the `predev` / `prebuild` hook in `apps/desktop/package.json`).
//
// Parsing is intentionally permissive — anything we can't classify is
// treated as a `changed` entry rather than blocking the build, so a
// malformed CHANGELOG.md never breaks `pnpm dev`.

import { readFile, writeFile } from "node:fs/promises";
import { argv, exit, stderr, stdout } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..");

const CHANGELOG_PATH = join(REPO_ROOT, "CHANGELOG.md");
const CONSTANTS_PATH = join(
	REPO_ROOT,
	"apps",
	"desktop",
	"src",
	"constants",
	"changelog.ts",
);
const REGION_START = "// RELEASES:START";
const REGION_END = "// RELEASES:END";

const KIND_BY_HEADING = new Map([
	["added", "added"],
	["new", "added"],
	["changed", "changed"],
	["updated", "changed"],
	["improved", "changed"],
	["fixed", "fixed"],
	["bug fixes", "fixed"],
	["deprecated", "deprecated"],
	["removed", "deprecated"],
]);

function tsString(s) {
	// Single-quote with backslash-escapes for backslash and apostrophe.
	// Smart quotes / em-dashes pass through unchanged so the rendered text
	// matches what's in CHANGELOG.md exactly.
	return `'${s.replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}

function parseChangelog(markdown) {
	const lines = markdown.split(/\r?\n/);
	const releases = [];
	let current = null;
	let currentKind = null;

	for (const line of lines) {
		// Version header: `## [<version>] — <date>` (em-dash) or `## [<version>] - <date>`.
		const versionMatch = line.match(
			/^##\s+\[([^\]]+)\](?:\s*[—-]\s*(.+))?\s*$/,
		);
		if (versionMatch) {
			const version = versionMatch[1].trim();
			const date = (versionMatch[2] ?? "").trim();
			if (version.toLowerCase() === "unreleased") {
				current = null;
				currentKind = null;
				continue;
			}
			current = {
				version,
				date,
				title: undefined,
				highlights: [],
				changes: [],
			};
			releases.push(current);
			currentKind = null;
			continue;
		}

		if (!current) continue;

		// Sub-heading: `### Added`, `### Highlights`, etc.
		const subMatch = line.match(/^###\s+(.+?)\s*$/);
		if (subMatch) {
			const heading = subMatch[1].trim().toLowerCase();
			if (heading === "highlights") {
				currentKind = "__highlights__";
			} else if (heading === "title") {
				currentKind = "__title__";
			} else {
				currentKind = KIND_BY_HEADING.get(heading) ?? null;
			}
			continue;
		}

		// Bullet line.
		const bulletMatch = line.match(/^\s*[-*]\s+(.+?)\s*$/);
		if (bulletMatch) {
			const text = collapseWhitespace(bulletMatch[1]);
			if (currentKind === "__highlights__") {
				current.highlights.push(text);
			} else if (currentKind === "__title__") {
				// Treat the first bullet under "### Title" as the title; uncommon.
				current.title ??= text;
			} else if (currentKind) {
				current.changes.push({ kind: currentKind, summary: text });
			} else {
				// Bullet with no kind heading above it → assume `changed`.
				current.changes.push({ kind: "changed", summary: text });
			}
			continue;
		}

		// Continuation line for a multi-line bullet.
		const continuation = line.match(/^\s{2,}(\S.*?)\s*$/);
		if (
			continuation &&
			currentKind &&
			currentKind !== "__highlights__" &&
			currentKind !== "__title__" &&
			current.changes.length > 0
		) {
			const last = current.changes[current.changes.length - 1];
			last.summary = collapseWhitespace(`${last.summary} ${continuation[1]}`);
			continue;
		}
	}

	return releases;
}

function collapseWhitespace(s) {
	return s.replace(/\s+/g, " ").trim();
}

function renderReleasesBlock(releases) {
	const out = [];
	out.push(REGION_START + " — auto-generated, do not edit by hand");
	out.push(
		"export const RELEASES: readonly ChangelogRelease[] = [",
	);
	for (const r of releases) {
		out.push("\t{");
		out.push(`\t\tversion: ${tsString(r.version)},`);
		out.push(`\t\tdate: ${tsString(r.date)},`);
		if (r.title) {
			out.push(`\t\ttitle: ${tsString(r.title)},`);
		}
		if (r.highlights.length > 0) {
			out.push("\t\thighlights: [");
			for (const h of r.highlights) {
				out.push(`\t\t\t${tsString(h)},`);
			}
			out.push("\t\t],");
		}
		out.push("\t\tchanges: [");
		for (const c of r.changes) {
			out.push(
				`\t\t\t{ kind: ${tsString(c.kind)}, summary: ${tsString(c.summary)} },`,
			);
		}
		out.push("\t\t],");
		out.push("\t},");
	}
	out.push("] as const;");
	out.push(REGION_END);
	return out.join("\n");
}

function spliceRegion(source, replacement) {
	const startIdx = source.indexOf(REGION_START);
	const endIdx = source.indexOf(REGION_END);
	if (startIdx === -1 || endIdx === -1 || endIdx < startIdx) {
		throw new Error(
			`Could not find ${REGION_START} … ${REGION_END} markers in ${CONSTANTS_PATH}`,
		);
	}
	const endLineEnd = source.indexOf("\n", endIdx);
	const tail = endLineEnd === -1 ? "" : source.slice(endLineEnd);
	return source.slice(0, startIdx) + replacement + tail;
}

async function main() {
	const args = argv.slice(2);
	const checkOnly = args.includes("--check");

	const markdown = await readFile(CHANGELOG_PATH, "utf8");
	const releases = parseChangelog(markdown);
	if (releases.length === 0) {
		stderr.write(
			"warn: no released versions found in CHANGELOG.md (only [Unreleased]?)\n",
		);
	}

	const before = await readFile(CONSTANTS_PATH, "utf8");
	const replacement = renderReleasesBlock(releases);
	const after = spliceRegion(before, replacement);

	if (before === after) {
		stdout.write("changelog.ts already in sync with CHANGELOG.md\n");
		return;
	}

	if (checkOnly) {
		stderr.write(
			"error: CHANGELOG.md and apps/desktop/src/constants/changelog.ts are out of sync. Run `pnpm sync:changelog`.\n",
		);
		exit(1);
	}

	await writeFile(CONSTANTS_PATH, after, "utf8");
	stdout.write(
		`synced ${releases.length} release(s) into apps/desktop/src/constants/changelog.ts\n`,
	);
}

main().catch((e) => {
	stderr.write(`sync-desktop-changelog: ${e.message}\n`);
	exit(1);
});
