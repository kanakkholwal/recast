#!/usr/bin/env node
// Extract the section for a given version from CHANGELOG.md.
//
// Usage:
//   node scripts/extract-changelog.mjs <version> [--file CHANGELOG.md] [--out body.md]
//
// <version> may be passed as "v0.1.2-beta" or "0.1.2-beta"; the leading "v"
// is stripped. The extracted block is the markdown between
// `## [<version>]` and the next `## [` heading, with the `## [...]` header
// itself omitted (the GitHub release UI already shows the version + tag).
//
// Exit codes:
//   0 — section found and printed (and written to --out if given)
//   2 — version not found in changelog
//   1 — usage / IO error

import { readFile, writeFile } from "node:fs/promises";
import { argv, exit, stderr, stdout } from "node:process";

function parseArgs(args) {
	const out = { version: null, file: "CHANGELOG.md", outPath: null };
	const rest = [];
	for (let i = 0; i < args.length; i++) {
		const a = args[i];
		if (a === "--file") out.file = args[++i];
		else if (a === "--out") out.outPath = args[++i];
		else if (a === "-h" || a === "--help") {
			stdout.write(
				"Usage: extract-changelog.mjs <version> [--file CHANGELOG.md] [--out body.md]\n",
			);
			exit(0);
		} else rest.push(a);
	}
	out.version = rest[0] ?? null;
	return out;
}

function normalize(v) {
	return v.replace(/^v/, "").trim();
}

function escapeRegex(s) {
	return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function extractSection(markdown, version) {
	const lines = markdown.split(/\r?\n/);
	const headerRe = /^##\s+\[([^\]]+)\]/;
	const target = version;
	let start = -1;
	let end = lines.length;
	for (let i = 0; i < lines.length; i++) {
		const m = lines[i].match(headerRe);
		if (!m) continue;
		if (start === -1 && m[1] === target) {
			start = i + 1;
		} else if (start !== -1) {
			end = i;
			break;
		}
	}
	if (start === -1) return null;
	return lines.slice(start, end).join("\n").trim();
}

const { version, file, outPath } = parseArgs(argv.slice(2));
if (!version) {
	stderr.write("error: version argument required\n");
	exit(1);
}

let markdown;
try {
	markdown = await readFile(file, "utf8");
} catch (e) {
	stderr.write(`error: cannot read ${file}: ${e.message}\n`);
	exit(1);
}

const v = normalize(version);
const body = extractSection(markdown, v);
if (!body) {
	stderr.write(`error: no section found for version ${v} in ${file}\n`);
	exit(2);
}

if (outPath) await writeFile(outPath, body + "\n", "utf8");
stdout.write(body + "\n");
