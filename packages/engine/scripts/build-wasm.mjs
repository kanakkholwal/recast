import { spawnSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { chmod, mkdir, readFile, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { createGunzip } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const repoRoot = resolve(pkgRoot, "../..");
const cratesRoot = join(repoRoot, "crates");
const toolsDir = join(repoRoot, ".tools");

const CRATE = "recast-ffi-wasm";
const WASM_NAME = "recast_ffi_wasm.wasm";

/** Each artifact ships exactly one backend: WebGL2 costs ~1.1 MB gzipped and a
 *  browser that has WebGPU never needs it. */
const VARIANTS = [
	{ feature: "webgpu", outName: "recast_engine_webgpu" },
	{ feature: "webgl2", outName: "recast_engine_webgl2" },
];

async function pinnedBindgenVersion() {
	const lock = await readFile(join(cratesRoot, "Cargo.lock"), "utf8");
	const match = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/);
	if (!match) throw new Error("wasm-bindgen is not in crates/Cargo.lock");
	return match[1];
}

function hostTriple() {
	const triples = {
		"win32:x64": "x86_64-pc-windows-msvc",
		"darwin:arm64": "aarch64-apple-darwin",
		"darwin:x64": "x86_64-apple-darwin",
		"linux:x64": "x86_64-unknown-linux-musl",
		"linux:arm64": "aarch64-unknown-linux-gnu",
	};
	const key = `${process.platform}:${process.arch}`;
	const triple = triples[key];
	if (!triple) throw new Error(`no wasm-bindgen release for ${key}`);
	return triple;
}

async function exists(path) {
	try {
		await stat(path);
		return true;
	} catch {
		return false;
	}
}

async function ensureBindgen(version) {
	const exe = join(toolsDir, process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen");
	if (await exists(exe)) {
		const found = spawnSync(exe, ["--version"], { encoding: "utf8" }).stdout?.trim();
		if (found === `wasm-bindgen ${version}`) return exe;
		await rm(exe, { force: true });
	}

	const triple = hostTriple();
	const url = `https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/wasm-bindgen-${version}-${triple}.tar.gz`;
	console.log(`fetching wasm-bindgen ${version} for ${triple}`);
	const response = await fetch(url);
	if (!response.ok) throw new Error(`download failed: ${response.status} ${url}`);

	await mkdir(toolsDir, { recursive: true });
	const tarball = join(toolsDir, "wasm-bindgen.tar");
	await pipeline(Readable.fromWeb(response.body), createGunzip(), createWriteStream(tarball));
	// `tar` is present on all three CI images and on Windows 10+.
	run(
		"tar",
		[
			"xf",
			tarball,
			"-C",
			toolsDir,
			"--strip-components=1",
			`wasm-bindgen-${version}-${triple}/${process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen"}`,
		],
		toolsDir,
	);
	await rm(tarball, { force: true });
	if (process.platform !== "win32") await chmod(exe, 0o755);
	return exe;
}

function run(command, args, cwd, env) {
	const result = spawnSync(command, args, {
		cwd,
		stdio: "inherit",
		shell: process.platform === "win32",
		env: env ? { ...process.env, ...env } : process.env,
	});
	if (result.status !== 0) {
		throw new Error(`${command} ${args.join(" ")} exited ${result.status ?? result.signal}`);
	}
}

const version = await pinnedBindgenVersion();
const bindgen = await ensureBindgen(version);
const outDir = join(pkgRoot, "wasm");
await mkdir(outDir, { recursive: true });

for (const { feature, outName } of VARIANTS) {
	run(
		"cargo",
		[
			"build",
			"-p",
			CRATE,
			"--target",
			"wasm32-unknown-unknown",
			"--release",
			"--no-default-features",
			"--features",
			feature,
		],
		cratesRoot,
		// REQUIRED, not a nicety. `VideoFrame` sits behind this cfg in web-sys, and
		// without it wgpu-hal's GLES backend compiles the VideoFrame texture upload
		// as `unimplemented!()` — so every decoded frame panics the moment the
		// engine lands on WebGL2. Appended so a RUSTFLAGS already in the
		// environment is not silently dropped.
		{ RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} --cfg=web_sys_unstable_apis`.trim() },
	);
	run(
		bindgen,
		[
			"--target",
			"web",
			// The generated `.d.ts` names every closure with a codegen hash that
			// changes on each build, so it can never be a committed artifact.
			// `wasm/*.d.ts` is hand-written against the same surface instead.
			"--no-typescript",
			"--out-dir",
			outDir,
			"--out-name",
			outName,
			join(cratesRoot, "target", "wasm32-unknown-unknown", "release", WASM_NAME),
		],
		repoRoot,
	);
	const bytes = (await stat(join(outDir, `${outName}_bg.wasm`))).size;
	console.log(`${outName}: ${(bytes / 1024 / 1024).toFixed(2)} MB`);
}
