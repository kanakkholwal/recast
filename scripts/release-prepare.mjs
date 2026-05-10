#!/usr/bin/env node
// Promote pending changesets into a new CHANGELOG.md section.
//
// Usage:
//   pnpm release:prepare <version>      # explicit version, e.g. "0.1.6" or "v0.1.6"
//   pnpm release:prepare --dry-run …    # preview without writing files
//
// What it does:
//   1. Reads `.changeset/*.md` (excluding README.md and config.json) and
//      parses YAML-ish frontmatter for the `recast-desktop` bump kind plus
//      the optional `kind:` field (added | changed | fixed | deprecated;
//      default: changed).
//   2. Validates the requested version is a sane bump from the latest
//      released version in CHANGELOG.md (warns on downgrades / skips, does
//      not block — final call is the maintainer's).
//   3. Builds a new `## [<version>] — <today>` section by merging:
//        a) anything currently sitting under `## [Unreleased]` in CHANGELOG.md
//        b) the entries collected from `.changeset/*.md`, grouped by kind.
//      Preserves any existing `### Highlights` block under [Unreleased].
//   4. Inserts that section above the previous topmost release, leaves a
//      fresh empty `## [Unreleased]` placeholder above it.
//   5. Deletes consumed changeset files (keeps README.md, config.json, and
//      any file starting with "_" so authors can stash drafts).
//   6. Re-runs `sync-desktop-changelog.mjs` so the desktop typed RELEASES
//      array reflects the new section immediately.
//
// What it does NOT do:
//   - Does not write source-file versions. The 0.0.0-0 placeholder
//     strategy means tauri.conf.json / Cargo.toml / package.json stay at
//     the placeholder; the release workflow rewrites them from the git tag.
//   - Does not commit, tag, or push. The maintainer reviews the diff and
//     does that themselves.

import { spawnSync } from "node:child_process";
import { readdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { argv, exit, stderr, stdout } from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..");

const CHANGELOG_PATH = join(REPO_ROOT, "CHANGELOG.md");
const CHANGESETS_DIR = join(REPO_ROOT, ".changeset");
const SYNC_SCRIPT = join(__dirname, "sync-desktop-changelog.mjs");

// Official SemVer 2.0.0 regex (semver.org). Anchored, so trailing junk like
// "1.2.3foo" or invalid pre-release identifiers are rejected.
const SEMVER_RE =
	/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

const KIND_VALUES = new Set(["added", "changed", "fixed", "deprecated"]);
const KIND_ORDER = ["added", "changed", "fixed", "deprecated"];
const KIND_HEADING = {
	added: "Added",
	changed: "Changed",
	fixed: "Fixed",
	deprecated: "Deprecated",
};

function parseArgs(args) {
	const out = { version: null, dryRun: false };
	for (const a of args) {
		if (a === "--dry-run" || a === "-n") out.dryRun = true;
		else if (a === "-h" || a === "--help") {
			stdout.write(
				"Usage: release-prepare.mjs <version> [--dry-run]\n",
			);
			exit(0);
		} else if (!out.version) out.version = a.replace(/^v/, "");
	}
	return out;
}

function parseFrontmatter(source) {
	const m = source.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
	if (!m) return { frontmatter: {}, body: source.trim() };
	const fm = {};
	for (const raw of m[1].split(/\r?\n/)) {
		const line = raw.trim();
		if (!line || line.startsWith("#")) continue;
		const idx = line.indexOf(":");
		if (idx === -1) continue;
		const key = line.slice(0, idx).trim().replace(/^["']|["']$/g, "");
		const val = line
			.slice(idx + 1)
			.trim()
			.replace(/^["']|["']$/g, "");
		fm[key] = val;
	}
	return { frontmatter: fm, body: m[2].trim() };
}

async function readChangesets() {
	let entries;
	try {
		entries = await readdir(CHANGESETS_DIR);
	} catch (e) {
		if (e.code === "ENOENT") return [];
		throw e;
	}
	const collected = [];
	for (const name of entries) {
		if (!name.endsWith(".md")) continue;
		if (name === "README.md") continue;
		if (name.startsWith("_")) continue;
		const file = join(CHANGESETS_DIR, name);
		const raw = await readFile(file, "utf8");
		const { frontmatter, body } = parseFrontmatter(raw);
		if (!body) continue;
		const bump = frontmatter["recast-desktop"];
		if (!bump) continue;
		let kind = (frontmatter.kind ?? "changed").toLowerCase();
		if (!KIND_VALUES.has(kind)) {
			stderr.write(
				`warn: ${name}: unknown kind "${kind}", defaulting to "changed"\n`,
			);
			kind = "changed";
		}
		collected.push({ file, name, bump, kind, summary: body.replace(/\s+/g, " ").trim() });
	}
	return collected;
}

function bumpRank(b) {
	return { patch: 1, minor: 2, major: 3 }[b] ?? 0;
}

function highestBump(changesets) {
	let top = null;
	for (const c of changesets) {
		if (bumpRank(c.bump) > bumpRank(top)) top = c.bump;
	}
	return top;
}

function findLatestReleasedVersion(markdown) {
	const lines = markdown.split(/\r?\n/);
	for (const line of lines) {
		const m = line.match(/^##\s+\[([^\]]+)\]/);
		if (!m) continue;
		if (m[1].toLowerCase() === "unreleased") continue;
		return m[1];
	}
	return null;
}

function suggestNextVersion(latest, bump) {
	if (!latest || !bump) return null;
	const m = latest.match(/^(\d+)\.(\d+)\.(\d+)(.*)$/);
	if (!m) return null;
	let [, maj, min, pat, suffix] = m;
	maj = Number(maj);
	min = Number(min);
	pat = Number(pat);
	if (bump === "major") return `${maj + 1}.0.0${suffix}`;
	if (bump === "minor") return `${maj}.${min + 1}.0${suffix}`;
	if (bump === "patch") return `${maj}.${min}.${pat + 1}${suffix}`;
	return null;
}

function isoToday() {
	const d = new Date();
	const yyyy = d.getFullYear();
	const mm = String(d.getMonth() + 1).padStart(2, "0");
	const dd = String(d.getDate()).padStart(2, "0");
	return `${yyyy}-${mm}-${dd}`;
}

function splitSections(markdown) {
	// Split CHANGELOG.md into { preamble, unreleased, releases } where
	// unreleased is the [Unreleased] block (header included) or null,
	// and releases is the rest of the file from the first non-Unreleased
	// `## [` onwards.
	const lines = markdown.split(/\r?\n/);
	let unreleasedStart = -1;
	let releasesStart = -1;
	for (let i = 0; i < lines.length; i++) {
		const m = lines[i].match(/^##\s+\[([^\]]+)\]/);
		if (!m) continue;
		if (m[1].toLowerCase() === "unreleased" && unreleasedStart === -1) {
			unreleasedStart = i;
		} else if (m[1].toLowerCase() !== "unreleased") {
			releasesStart = i;
			break;
		}
	}
	const preambleEnd =
		unreleasedStart !== -1
			? unreleasedStart
			: releasesStart !== -1
				? releasesStart
				: lines.length;
	const preamble = lines.slice(0, preambleEnd).join("\n");
	const unreleasedLines =
		unreleasedStart === -1
			? []
			: lines.slice(
					unreleasedStart,
					releasesStart === -1 ? lines.length : releasesStart,
				);
	const releases = (
		releasesStart === -1 ? "" : lines.slice(releasesStart).join("\n")
	).replace(/\s+$/, "");
	return { preamble, unreleased: unreleasedLines.join("\n"), releases };
}

function parseUnreleasedBlock(unreleasedMd) {
	// Returns { highlights: string[], byKind: { added: string[], ... }, leftovers: string[] }
	const result = {
		highlights: [],
		byKind: { added: [], changed: [], fixed: [], deprecated: [] },
		leftovers: [],
	};
	if (!unreleasedMd) return result;
	const lines = unreleasedMd.split(/\r?\n/);
	let mode = null;
	for (const line of lines) {
		if (/^##\s+\[/.test(line)) continue;
		const sub = line.match(/^###\s+(.+?)\s*$/);
		if (sub) {
			const heading = sub[1].trim().toLowerCase();
			if (heading === "highlights") mode = "highlights";
			else if (heading === "added" || heading === "new") mode = "added";
			else if (
				heading === "changed" ||
				heading === "updated" ||
				heading === "improved"
			)
				mode = "changed";
			else if (heading === "fixed" || heading === "bug fixes") mode = "fixed";
			else if (heading === "deprecated" || heading === "removed")
				mode = "deprecated";
			else mode = null;
			continue;
		}
		const bullet = line.match(/^\s*[-*]\s+(.+)$/);
		if (bullet) {
			const text = bullet[1].trim();
			if (mode === "highlights") result.highlights.push(text);
			else if (mode && KIND_VALUES.has(mode)) result.byKind[mode].push(text);
			else result.leftovers.push(text);
			continue;
		}
		// Continuation line for the previous bullet.
		const cont = line.match(/^\s{2,}(\S.*?)\s*$/);
		if (cont) {
			const buckets =
				mode === "highlights"
					? result.highlights
					: mode && KIND_VALUES.has(mode)
						? result.byKind[mode]
						: result.leftovers;
			if (buckets.length > 0)
				buckets[buckets.length - 1] = `${buckets[buckets.length - 1]} ${cont[1].trim()}`;
		}
	}
	return result;
}

function renderSection({ version, date, highlights, byKind }) {
	const out = [];
	out.push(`## [${version}] — ${date}`);
	out.push("");
	if (highlights.length > 0) {
		out.push("### Highlights");
		for (const h of highlights) out.push(`- ${h}`);
		out.push("");
	}
	for (const kind of KIND_ORDER) {
		const items = byKind[kind];
		if (!items || items.length === 0) continue;
		out.push(`### ${KIND_HEADING[kind]}`);
		for (const it of items) out.push(`- ${it}`);
		out.push("");
	}
	return out.join("\n").trim() + "\n";
}

function renderEmptyUnreleased() {
	return "## [Unreleased]\n";
}

function mergeEntries(unreleased, changesets) {
	const byKind = {
		added: [...unreleased.byKind.added],
		changed: [...unreleased.byKind.changed, ...unreleased.leftovers],
		fixed: [...unreleased.byKind.fixed],
		deprecated: [...unreleased.byKind.deprecated],
	};
	for (const c of changesets) byKind[c.kind].push(c.summary);
	// Stable de-duplication: keep first occurrence.
	for (const k of KIND_ORDER) {
		const seen = new Set();
		byKind[k] = byKind[k].filter((s) => {
			if (seen.has(s)) return false;
			seen.add(s);
			return true;
		});
	}
	return { highlights: [...unreleased.highlights], byKind };
}

async function main() {
	const { version, dryRun } = parseArgs(argv.slice(2));
	if (!version) {
		stderr.write("error: version argument required (e.g. `pnpm release:prepare 0.1.6`)\n");
		exit(1);
	}
	if (!SEMVER_RE.test(version)) {
		stderr.write(`error: "${version}" doesn't look like SemVer\n`);
		exit(1);
	}

	const changesets = await readChangesets();
	const markdown = await readFile(CHANGELOG_PATH, "utf8");
	const latest = findLatestReleasedVersion(markdown);
	const top = highestBump(changesets);
	const suggested = suggestNextVersion(latest, top);

	stdout.write(
		`Latest released: ${latest ?? "(none)"}    Pending changesets: ${changesets.length}    Highest bump: ${top ?? "(none)"}\n`,
	);
	if (suggested && suggested !== version) {
		stderr.write(
			`warn: changesets suggest next version is ${suggested}; you asked for ${version}. Proceeding.\n`,
		);
	}

	const { preamble, unreleased, releases } = splitSections(markdown);
	const parsedUnreleased = parseUnreleasedBlock(unreleased);
	const merged = mergeEntries(parsedUnreleased, changesets);

	const totalEntries =
		KIND_ORDER.reduce((sum, k) => sum + merged.byKind[k].length, 0) +
		merged.highlights.length;
	if (totalEntries === 0) {
		stderr.write(
			"error: nothing to release — no [Unreleased] entries and no changesets found.\n",
		);
		exit(1);
	}

	const newSection = renderSection({
		version,
		date: isoToday(),
		highlights: merged.highlights,
		byKind: merged.byKind,
	});

	const next =
		preamble.replace(/\s+$/, "") +
		"\n\n" +
		renderEmptyUnreleased() +
		"\n" +
		newSection +
		"\n" +
		releases +
		"\n";

	if (dryRun) {
		stdout.write("\n--- new section ---\n");
		stdout.write(newSection);
		stdout.write("\n--- (dry run; no files written) ---\n");
		return;
	}

	await writeFile(CHANGELOG_PATH, next, "utf8");
	stdout.write(`wrote new section [${version}] to CHANGELOG.md\n`);

	const sync = spawnSync(process.execPath, [SYNC_SCRIPT], {
		stdio: "inherit",
	});
	if (sync.status !== 0) {
		stderr.write(
			"error: sync-desktop-changelog.mjs failed; CHANGELOG.md was updated but desktop constants were not. Changesets left in place so this run can be retried.\n",
		);
		exit(sync.status ?? 1);
	}

	for (const c of changesets) {
		await rm(c.file);
		stdout.write(`removed ${c.name}\n`);
	}

	stdout.write(
		`\nNext: review the diff, commit (CHANGELOG.md + apps/desktop/src/constants/changelog.ts + .changeset), then tag v${version} and push.\n`,
	);
}

main().catch((e) => {
	stderr.write(`release-prepare: ${e.message}\n`);
	exit(1);
});
