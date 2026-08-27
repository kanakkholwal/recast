/**
 * Verify every Recast extension pack under `extensions/packs/*`.
 *
 * This is the gate that lets the registry accept community PRs safely. It runs
 * with **zero dependencies** (Node built-ins only) so CI needs no install step.
 *
 * For each pack it validates, collecting ALL problems before failing:
 *   - manifest shape (required fields, types, kind, semver, empty permissions),
 *   - id is a path-safe slug AND equals the pack's folder name,
 *   - every asset file exists, sits in `assets/`, and has a bare, safe filename
 *     (no traversal / drive prefix / Windows device name),
 *   - every contribution references a declared asset id, and that asset's file
 *     type matches its use (cursor -> SVG, background -> raster image),
 *   - no unreferenced ("dead") assets,
 *   - contribution ids are unique within a kind, asset ids are unique,
 *   - computes each asset's SHA-256 (the value the installable manifest pins).
 *
 * Exit code is non-zero if ANY pack fails.
 *
 * IMPORTANT: the filename / id safety rules below MUST stay in sync with the
 * desktop installer's gate in
 * `apps/desktop/src-tauri/src/commands/extensions.rs` - they are the same trust
 * boundary enforced in two places (CI at submission, Rust at install).
 *
 * Usage:  node extensions/scripts/verify-extensions.mjs
 */

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const EXT_ROOT = resolve(SCRIPTS_DIR, "..");
const PACKS_DIR = join(EXT_ROOT, "packs");

const RESERVED_NAMES = new Set([
	"CON",
	"PRN",
	"AUX",
	"NUL",
	"COM1",
	"COM2",
	"COM3",
	"COM4",
	"COM5",
	"COM6",
	"COM7",
	"COM8",
	"COM9",
	"LPT1",
	"LPT2",
	"LPT3",
	"LPT4",
	"LPT5",
	"LPT6",
	"LPT7",
	"LPT8",
	"LPT9",
]);

const CURSOR_EXTS = new Set([".svg"]);
const IMAGE_EXTS = new Set([".png", ".jpg", ".jpeg", ".webp"]);

/** True if `s` contains a control character (U+0000..U+001F or U+007F).
 *  Mirrors Rust `char::is_control`. Uses code points (no control bytes in
 *  this source file) so the script stays a clean text file. */
function hasControlChar(s) {
	for (let i = 0; i < s.length; i++) {
		const c = s.charCodeAt(i);
		if (c < 0x20 || c === 0x7f) return true;
	}
	return false;
}

/** Mirrors `is_safe_filename` in extensions.rs. */
function isSafeFilename(name) {
	if (!name || name.length > 255) return false;
	if (name.includes("/") || name.includes("\\") || name.includes("..")) return false;
	if (hasControlChar(name)) return false;
	if (name[1] === ":") return false; // drive prefix
	if (name.startsWith(".") || name.startsWith(" ")) return false;
	if (name.endsWith(".") || name.endsWith(" ")) return false;
	const stem = name.split(".")[0].toUpperCase();
	return !RESERVED_NAMES.has(stem);
}

/** Mirrors `is_safe_ext_id` in extensions.rs. */
function isSafeExtId(id) {
	return (
		typeof id === "string" &&
		id.length > 0 &&
		id.length <= 64 &&
		id !== "." &&
		id !== ".." &&
		!id.startsWith(".") &&
		/^[A-Za-z0-9._-]+$/.test(id)
	);
}

const isSemver = (v) => typeof v === "string" && /^\d+\.\d+\.\d+$/.test(v);
const ext = (file) => {
	const b = basename(file);
	const i = b.lastIndexOf(".");
	return i < 0 ? "" : b.slice(i).toLowerCase();
};
const sha256 = (buf) => createHash("sha256").update(buf).digest("hex");

const isSvg = (text) => /^\s*(?:<\?xml[\s\S]*?\?>\s*)?(?:<!--[\s\S]*?-->\s*)*<svg[\s>]/.test(text);

/** Collects all problems for one pack, then reports pass/fail. */
class PackResult {
	constructor(id) {
		this.id = id;
		this.errors = [];
		this.assets = []; // { id, filename, sha256, bytes }
	}
	err(msg) {
		this.errors.push(msg);
	}
	get ok() {
		return this.errors.length === 0;
	}
}

function listPackDirs() {
	if (!existsSync(PACKS_DIR)) return [];
	return readdirSync(PACKS_DIR)
		.map((name) => join(PACKS_DIR, name))
		.filter((p) => {
			try {
				return statSync(p).isDirectory();
			} catch {
				return false;
			}
		});
}

const isStr = (v) => typeof v === "string" && v.length > 0;

function validateHotspot(res, where, h) {
	if (h == null || typeof h !== "object") {
		res.err(`${where}: hotspot must be an object { x, y }`);
		return;
	}
	for (const k of ["x", "y"]) {
		if (typeof h[k] !== "number" || h[k] < 0 || h[k] > 64) {
			res.err(`${where}: hotspot.${k} must be a number in 0..64`);
		}
	}
}

/**
 * Walk all contributions, validating their own fields and recording how each
 * referenced asset id is *used* (so the file type can be checked later).
 * Returns Map<assetId, "cursor"|"image"> of expected types.
 */
function validateContributions(res, contributes) {
	const expectedType = new Map();
	const seenByKind = new Map(); // kind -> Set(ids)

	const claimId = (kind, id, where) => {
		// An invalid/missing id isn't worth uniqueness tracking — doing so would
		// produce noisy follow-ups like `duplicate cursors id "undefined"`.
		if (!isSafeExtId(id)) {
			res.err(`${where}: id "${id}" must be a slug [A-Za-z0-9._-]`);
			return;
		}
		const set = seenByKind.get(kind) ?? new Set();
		if (set.has(id)) res.err(`${where}: duplicate ${kind} id "${id}"`);
		set.add(id);
		seenByKind.set(kind, set);
	};
	const ref = (assetId, type, where) => {
		if (!isStr(assetId)) {
			res.err(`${where}: missing asset reference`);
			return;
		}
		const prev = expectedType.get(assetId);
		if (prev && prev !== type) {
			res.err(`${where}: asset "${assetId}" used as both ${prev} and ${type}`);
		}
		expectedType.set(assetId, type);
	};

	const c = contributes ?? {};
	const kinds = [
		"cursors",
		"backgrounds",
		"gradients",
		"colors",
		"easings",
		"smoothings",
		"captionPresets",
	];
	for (const u of Object.keys(c).filter((k) => !kinds.includes(k))) {
		res.err(`contributes.${u}: unknown contribution kind`);
	}
	let total = 0;

	for (const cur of c.cursors ?? []) {
		total++;
		const w = `cursor "${cur?.id}"`;
		claimId("cursors", cur?.id, w);
		if (!isStr(cur?.label)) res.err(`${w}: label is required`);
		ref(cur?.rest, "cursor", `${w}.rest`);
		if (cur?.press != null) ref(cur.press, "cursor", `${w}.press`);
		if (cur?.rightPress != null) ref(cur.rightPress, "cursor", `${w}.rightPress`);
		if (cur?.drag != null) ref(cur.drag, "cursor", `${w}.drag`);
		validateHotspot(res, w, cur?.hotspot);
		if (cur?.pressedHotspot != null)
			validateHotspot(res, `${w}.pressedHotspot`, cur.pressedHotspot);
		if (cur?.rightPressedHotspot != null)
			validateHotspot(res, `${w}.rightPressedHotspot`, cur.rightPressedHotspot);
		if (cur?.dragHotspot != null) validateHotspot(res, `${w}.dragHotspot`, cur.dragHotspot);
	}
	for (const b of c.backgrounds ?? []) {
		total++;
		const w = `background "${b?.id}"`;
		claimId("backgrounds", b?.id, w);
		if (!isStr(b?.label)) res.err(`${w}: label is required`);
		ref(b?.asset, "image", `${w}.asset`);
		if (b?.thumb != null) ref(b.thumb, "image", `${w}.thumb`);
	}
	for (const g of c.gradients ?? []) {
		total++;
		const w = `gradient "${g?.id}"`;
		claimId("gradients", g?.id, w);
		if (!isStr(g?.label)) res.err(`${w}: label is required`);
		if (!isStr(g?.value) || !g.value.includes("gradient(")) {
			res.err(`${w}: value must be a CSS gradient() string`);
		}
	}
	for (const col of c.colors ?? []) {
		total++;
		const w = `color "${col?.id}"`;
		claimId("colors", col?.id, w);
		if (!isStr(col?.label)) res.err(`${w}: label is required`);
		if (!/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(col?.value ?? "")) {
			res.err(`${w}: value must be a hex colour`);
		}
	}
	for (const e of c.easings ?? []) {
		total++;
		const w = `easing "${e?.id}"`;
		claimId("easings", e?.id, w);
		if (!isStr(e?.label)) res.err(`${w}: label is required`);
		const v = e?.value;
		if (!v || ["x1", "y1", "x2", "y2"].some((k) => typeof v[k] !== "number")) {
			res.err(`${w}: value must be { x1, y1, x2, y2 } numbers`);
		}
	}
	for (const s of c.smoothings ?? []) {
		total++;
		const w = `smoothing "${s?.id}"`;
		claimId("smoothings", s?.id, w);
		if (!isStr(s?.label)) res.err(`${w}: label is required`);
		if (typeof s?.smoothing !== "number" || s.smoothing < 0 || s.smoothing > 100) {
			res.err(`${w}: smoothing must be 0..100`);
		}
		if (typeof s?.snapToClicks !== "boolean") res.err(`${w}: snapToClicks must be boolean`);
		if (typeof s?.snapWindowMs !== "number" || s.snapWindowMs < 0) {
			res.err(`${w}: snapWindowMs must be a non-negative number`);
		}
	}

	// Caption themes carry their whole style in the manifest and reference no
	// assets — validate the fields + ranges (mirrors the JSON schema). Kept in
	// sync with `CaptionPresetValue` in apps/desktop/src/lib/registry/types.ts.
	const isHex = (v) => /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(v ?? "");
	const inRange = (v, lo, hi) => typeof v === "number" && v >= lo && v <= hi;
	for (const p of c.captionPresets ?? []) {
		total++;
		const w = `captionPreset "${p?.id}"`;
		claimId("captionPresets", p?.id, w);
		if (!isStr(p?.label)) res.err(`${w}: label is required`);
		if (!isStr(p?.fontFamily)) res.err(`${w}: fontFamily is required`);
		if (!inRange(p?.fontWeight, 100, 900)) res.err(`${w}: fontWeight must be 100..900`);
		if (!inRange(p?.fontSizePct, 1, 20)) res.err(`${w}: fontSizePct must be 1..20`);
		if (!["top", "center", "bottom"].includes(p?.position)) {
			res.err(`${w}: position must be top|center|bottom`);
		}
		if (!["left", "center", "right"].includes(p?.align)) {
			res.err(`${w}: align must be left|center|right`);
		}
		if (!inRange(p?.offsetPct, 0, 40)) res.err(`${w}: offsetPct must be 0..40`);
		if (!isHex(p?.color)) res.err(`${w}: color must be a hex colour`);
		if (typeof p?.uppercase !== "boolean") res.err(`${w}: uppercase must be boolean`);
		if (!inRange(p?.letterSpacing, -0.2, 1)) res.err(`${w}: letterSpacing must be -0.2..1`);
		if (!["none", "soft", "box"].includes(p?.background)) {
			res.err(`${w}: background must be none|soft|box`);
		}
		if (!isHex(p?.backgroundColor)) res.err(`${w}: backgroundColor must be a hex colour`);
		if (!inRange(p?.backgroundOpacity, 0, 100)) res.err(`${w}: backgroundOpacity must be 0..100`);
		if (!inRange(p?.outlineWidth, 0, 20)) res.err(`${w}: outlineWidth must be 0..20`);
		if (!isHex(p?.outlineColor)) res.err(`${w}: outlineColor must be a hex colour`);
		if (!Number.isInteger(p?.maxLines) || p.maxLines < 1 || p.maxLines > 6) {
			res.err(`${w}: maxLines must be an integer 1..6`);
		}
	}

	if (total === 0) res.err("contributes: a pack must contribute at least one item");
	return expectedType;
}

function validatePack(dir) {
	const folder = basename(dir);
	const res = new PackResult(folder);
	const manifestPath = join(dir, "extension.json");

	if (!existsSync(manifestPath)) {
		res.err("missing extension.json");
		return res;
	}

	let m;
	try {
		m = JSON.parse(readFileSync(manifestPath, "utf8"));
	} catch (e) {
		res.err(`extension.json is not valid JSON: ${e.message}`);
		return res;
	}

	// Envelope
	if (!isSafeExtId(m.id)) res.err(`id "${m.id}" is not a path-safe slug`);
	else if (m.id !== folder) res.err(`id "${m.id}" must equal the folder name "${folder}"`);
	if (!isStr(m.name)) res.err("name is required");
	if (!isSemver(m.version)) res.err(`version "${m.version}" must be semver x.y.z`);
	if (m.kind !== "asset-pack") res.err(`kind must be "asset-pack" (got "${m.kind}")`);
	if (!Array.isArray(m.permissions) || m.permissions.length !== 0) {
		res.err("permissions must be an empty array (asset-packs run no code)");
	}
	if (m.contributes == null || typeof m.contributes !== "object") {
		res.err("contributes is required");
	}
	if (!Array.isArray(m.assets)) {
		res.err("assets must be an array");
		return res;
	}

	const expectedType = validateContributions(res, m.contributes ?? {});

	// Assets
	const declared = new Map(); // id -> asset
	for (const a of m.assets) {
		const w = `asset "${a?.id}"`;
		if (!isSafeExtId(a?.id)) {
			res.err(`${w}: id must be a slug`);
			continue;
		}
		if (declared.has(a.id)) res.err(`${w}: duplicate asset id`);
		declared.set(a.id, a);

		if (!isStr(a.file) || !/^assets\/[^/\\]+$/.test(a.file)) {
			res.err(`${w}: file must be a pack-relative "assets/<name>" path`);
			continue;
		}
		const fname = basename(a.file);
		if (!isSafeFilename(fname)) {
			res.err(`${w}: unsafe filename "${fname}"`);
			continue;
		}
		const abs = join(dir, a.file);
		if (!existsSync(abs)) {
			res.err(`${w}: file not found at ${a.file}`);
			continue;
		}

		let buf;
		try {
			buf = readFileSync(abs);
		} catch (e) {
			// A directory, broken symlink, or unreadable file is a per-asset
			// problem — record it and keep verifying so CI reports every issue.
			res.err(`${w}: could not read file ${a.file}: ${e.message}`);
			continue;
		}
		const type = expectedType.get(a.id);
		const e = ext(fname);
		if (!type) {
			res.err(`${w}: declared but never referenced by any contribution`);
		} else if (type === "cursor") {
			if (!CURSOR_EXTS.has(e)) res.err(`${w}: cursor asset must be .svg`);
			else if (!isSvg(buf.toString("utf8"))) res.err(`${w}: not a valid SVG`);
		} else if (type === "image" && !IMAGE_EXTS.has(e))
			res.err(`${w}: background asset must be .png/.jpg/.jpeg/.webp`);

		res.assets.push({ id: a.id, filename: fname, sha256: sha256(buf), bytes: buf.length });
	}

	// Every referenced asset id must be declared.
	for (const refId of expectedType.keys()) {
		if (!declared.has(refId)) res.err(`contribution references undeclared asset id "${refId}"`);
	}

	return res;
}

// - main -

const dirs = listPackDirs();
if (dirs.length === 0) {
	console.log("No extension packs found under extensions/packs/. Nothing to verify.");
	process.exit(0);
}

let failed = 0;
console.log(`Verifying ${dirs.length} extension pack(s)...\n`);
for (const dir of dirs) {
	const res = validatePack(dir);
	if (res.ok) {
		const kb = (res.assets.reduce((n, a) => n + a.bytes, 0) / 1024).toFixed(1);
		console.log(`PASS  ${res.id}  (${res.assets.length} asset(s), ${kb} KB)`);
		for (const a of res.assets) {
			console.log(`        ${a.filename}  sha256:${a.sha256.slice(0, 12)}`);
		}
	} else {
		failed++;
		console.log(`FAIL  ${res.id}`);
		for (const e of res.errors) console.log(`        - ${e}`);
	}
}

console.log("");
if (failed > 0) {
	console.error(`${failed} pack(s) failed verification.`);
	process.exit(1);
}
console.log("All extension packs are valid.");
