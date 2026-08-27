#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const EXTS = /\.(ts|tsx|js|mjs|cjs|svelte|css|rs)$/;

/** Tool directives are instructions, not prose, and may sit in a run. */
const DIRECTIVE =
	/^\s*(\/\/|\/\*)\s*(biome-ignore|eslint-|@ts-|svelte-ignore|prettier-|deno-|oxlint-|c8\s|istanbul\s|SPDX-|cspell|codegen|@vitest-|v8\s)/;

const SHEBANG = /^#!/;

/** Sentinel base meaning "whatever is in the index", for the pre-commit hook. */
const STAGED = Symbol.for("staged");

function git(args) {
	return execFileSync("git", args, { encoding: "utf8", maxBuffer: 256 * 1024 * 1024 })
		.split("\n")
		.filter(Boolean);
}

function mergeBase(since) {
	try {
		return execFileSync("git", ["merge-base", "HEAD", since], { encoding: "utf8" }).trim() || since;
	} catch {
		return since;
	}
}

function targetFiles(base) {
	if (base === STAGED)
		return git(["diff", "--cached", "--name-only", "--diff-filter=ACMR"]).filter((f) =>
			EXTS.test(f),
		);
	if (!base) return git(["ls-files"]).filter((f) => EXTS.test(f));
	const changed = git(["diff", "--name-only", "--diff-filter=ACMR", base, "--"]);
	// A brand-new file is absent from `git diff`, so it would ship unchecked.
	const untracked = git(["ls-files", "--others", "--exclude-standard"]);
	return [...new Set([...changed, ...untracked])].filter((f) => EXTS.test(f));
}

/** Line numbers this change added or rewrote, so untouched prose is not our problem. */
function addedLines(base, file) {
	const range =
		base === STAGED ? ["diff", "--cached", "-U0", "--", file] : ["diff", "-U0", base, "--", file];
	const hunks = git(range).filter((l) => l.startsWith("@@"));
	// No hunks means the file is untracked: all of it is new.
	if (hunks.length === 0) return null;
	const added = new Set();
	for (const h of hunks) {
		const m = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(h);
		if (!m) continue;
		const start = Number(m[1]);
		const count = m[2] === undefined ? 1 : Number(m[2]);
		for (let i = 0; i < count; i++) added.add(start + i);
	}
	return added;
}

/** Rust `///`+`//!` are the rustdoc analogue of JSDoc, so they are exempt. */
function isDocLine(trimmed, ext) {
	return ext === "rs" && (trimmed.startsWith("///") || trimmed.startsWith("//!"));
}

function scopeLabel() {
	if (base === STAGED) return "in staged changes";
	return since ? `added since ${since}` : "in the repo";
}

function overlaps(v, touched) {
	for (let l = v.line; l <= v.endLine; l++) if (touched.has(l)) return true;
	return false;
}

function scan(file, text) {
	const ext = file.split(".").pop();
	const lines = text.split(/\r?\n/);
	const out = [];

	let run = 0;
	let start = 0;
	const flush = (endLine) => {
		if (run > 1) out.push({ line: start, endLine, kind: `${run}-line // comment` });
		run = 0;
	};
	for (let i = 0; i < lines.length; i++) {
		const t = lines[i].trim();
		const isComment =
			t.startsWith("//") && !isDocLine(t, ext) && !DIRECTIVE.test(lines[i]) && !SHEBANG.test(t);
		if (isComment) {
			if (run === 0) start = i + 1;
			run++;
		} else {
			flush(i);
		}
	}
	flush(lines.length);

	// Block comments: /** */ is JSDoc and exempt, plain /* */ is not.
	const re = /\/\*[\s\S]*?\*\//g;
	let m;
	while ((m = re.exec(text)) !== null) {
		if (m[0].startsWith("/**") || !m[0].includes("\n")) continue;
		if (DIRECTIVE.test(m[0])) continue;
		const line = text.slice(0, m.index).split(/\r?\n/).length;
		const span = m[0].split(/\r?\n/).length;
		out.push({ line, endLine: line + span - 1, kind: `${span}-line /* */ block` });
	}
	return out;
}

const args = process.argv.slice(2);
const sinceArg = args.find((a) => a.startsWith("--since="));
const staged = args.includes("--staged");
const since = args.includes("--all") ? null : (sinceArg?.slice(8) ?? "origin/main");
const base = staged ? STAGED : since ? mergeBase(since) : null;

const files = targetFiles(base);
let total = 0;
for (const file of files) {
	let text;
	try {
		text = readFileSync(file, "utf8");
	} catch {
		continue;
	}
	// Only comments this change actually wrote: a ratchet, not a repo-wide sweep.
	const touched = base ? addedLines(base, file) : null;
	for (const v of scan(file, text)) {
		if (touched && !overlaps(v, touched)) continue;
		console.error(
			`${file}:${v.line}  ${v.kind} (max 1; only JSDoc /** */ and rustdoc /// may span)`,
		);
		total++;
	}
}

if (total > 0) {
	console.error(`\n${total} multi-line comment${total === 1 ? "" : "s"} ${scopeLabel()}.`);
	console.error("Say it in one line, or make the code say it. JSDoc and rustdoc are exempt.");
	process.exit(1);
}
console.log(`comment check: ${files.length} file(s) clean`);
