import { spawnSync } from "node:child_process";
import { readdir, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Build the wasm artifacts if they are missing or older than their sources.
 *
 * The artifacts are gitignored build products, so a fresh clone has none, and
 * `tauri build` runs `ui:build` directly rather than through turbo, which means
 * turbo's dependency graph never gets a chance to build them first. Vite then
 * fails to resolve `../wasm/recast_engine_webgpu.js` and the whole desktop build
 * dies. Calling this from `ui:build` is what makes any entry point work.
 */

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const repoRoot = resolve(pkgRoot, "../..");
const cratesRoot = join(repoRoot, "crates");
const outDir = join(pkgRoot, "wasm");

const ARTIFACTS = [
	"recast_engine_webgpu.js",
	"recast_engine_webgpu_bg.wasm",
	"recast_engine_webgl2.js",
	"recast_engine_webgl2_bg.wasm",
];

/** Files whose change should force a rebuild. */
const SOURCE_EXTENSIONS = [".rs", ".wgsl", ".toml", ".lock"];

async function newestSourceTime(dir) {
	let newest = 0;
	let entries;
	try {
		entries = await readdir(dir, { withFileTypes: true });
	} catch {
		return newest;
	}
	for (const entry of entries) {
		// `target` is where cargo writes; walking it costs seconds and tells us
		// nothing about whether the inputs moved.
		if (entry.name === "target" || entry.name === ".git") continue;
		const path = join(dir, entry.name);
		if (entry.isDirectory()) {
			newest = Math.max(newest, await newestSourceTime(path));
		} else if (SOURCE_EXTENSIONS.some((ext) => entry.name.endsWith(ext))) {
			newest = Math.max(newest, (await stat(path)).mtimeMs);
		}
	}
	return newest;
}

async function oldestArtifactTime() {
	let oldest = Number.POSITIVE_INFINITY;
	for (const name of ARTIFACTS) {
		try {
			oldest = Math.min(oldest, (await stat(join(outDir, name))).mtimeMs);
		} catch {
			return 0;
		}
	}
	return oldest;
}

const artifacts = await oldestArtifactTime();
if (artifacts === 0) {
	console.log("[ensure-wasm] artifacts missing; building");
} else if (artifacts < (await newestSourceTime(cratesRoot))) {
	console.log("[ensure-wasm] crate sources are newer than the artifacts; rebuilding");
} else {
	console.log("[ensure-wasm] up to date");
	process.exit(0);
}

const result = spawnSync(process.execPath, [join(here, "build-wasm.mjs")], {
	stdio: "inherit",
	cwd: pkgRoot,
});
process.exit(result.status ?? 1);
