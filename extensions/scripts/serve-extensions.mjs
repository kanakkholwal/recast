/**
 * Local extension registry dev server.
 *
 * Builds every pack (via `buildRegistry`) pointed at this server's own URL,
 * serves the result **from memory**, and watches `extensions/` to rebuild on
 * any source change — so you can iterate on a pack and re-install it in the app
 * without restarting anything.
 *
 * Memory-safe by construction: each rebuild replaces the in-memory snapshot
 * wholesale (the previous Buffers become garbage), and the watcher + HTTP server
 * are closed cleanly on Ctrl-C / SIGTERM. Nothing accumulates across rebuilds.
 *
 * Usage:
 *   pnpm serve:extensions            # http://localhost:4422
 *   PORT=9000 pnpm serve:extensions  # pick a port
 *
 * Then in the app → Extensions → Install from URL:
 *   http://localhost:4422/<packId>.extension.json
 * or point the browse gallery at:
 *   http://localhost:4422/index.json
 */

import { readdirSync, readFileSync, statSync, watch } from "node:fs";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { buildRegistry, DIST_DIR, EXT_ROOT } from "./build-registry.mjs";

const PORT = 4422;
const BASE_URL = `http://localhost:${PORT}`.replace(/\/+$/, "");
const DEBOUNCE_MS = 150;

const CONTENT_TYPES = {
	".json": "application/json",
	".svg": "image/svg+xml",
	".png": "image/png",
	".jpg": "image/jpeg",
	".jpeg": "image/jpeg",
	".webp": "image/webp",
};

/** url path -> { body: Buffer, type } */
let snapshot = new Map();

/** Read the freshly-built dist/ into an in-memory map (replaces the old one). */
function loadDistIntoMemory() {
	const next = new Map();
	for (const name of readdirSync(DIST_DIR)) {
		const abs = join(DIST_DIR, name);
		if (!statSync(abs).isFile()) continue;
		next.set(`/${name}`, {
			body: readFileSync(abs),
			type: CONTENT_TYPES[extname(name).toLowerCase()] ?? "application/octet-stream",
		});
	}
	snapshot = next; // old snapshot (and its Buffers) is now garbage
}

function rebuild(reason) {
	try {
		const { entries } = buildRegistry({ baseUrl: BASE_URL });
		loadDistIntoMemory();
		const ids = entries.map((e) => e.id).join(", ") || "—";
		console.log(`[serve] ${reason} → ${entries.length} pack(s) [${ids}], ${snapshot.size} file(s)`);
	} catch (err) {
		console.error(`[serve] build failed (${reason}): ${err.message}`);
	}
}

rebuild("initial build");

const server = createServer((req, res) => {
	// `decodeURIComponent` throws on malformed percent-encoding, and one bad request must not take down the server loop.
	let path;
	try {
		path = decodeURIComponent((req.url ?? "/").split("?")[0]);
	} catch {
		res.writeHead(400, { "content-type": "text/plain" });
		res.end("bad request");
		return;
	}
	const key = path === "/" ? "/index.json" : path;
	const hit = snapshot.get(key);
	if (!hit) {
		res.writeHead(404, { "content-type": "text/plain" });
		res.end("not found");
		return;
	}
	res.writeHead(200, {
		"content-type": hit.type,
		"content-length": hit.body.length,
		"access-control-allow-origin": "*",
		"cache-control": "no-store",
	});
	res.end(req.method === "HEAD" ? undefined : hit.body);
});

server.on("error", (err) => {
	if (err.code === "EADDRINUSE") {
		console.error(`[serve] port ${PORT} is in use. Set PORT=<n> to pick another.`);
	} else {
		console.error(`[serve] server error: ${err.message}`);
	}
	process.exit(1);
});

server.listen(PORT, () => {
	console.log(`[serve] registry on http://localhost:${PORT}  (base ${BASE_URL})`);
	console.log(`[serve] install from http://localhost:${PORT}/<packId>.extension.json`);
	console.log(`[serve] watching ${EXT_ROOT} for changes — Ctrl-C to stop`);
});

// Watch sources recursively; ignore our own writes under dist/.
let timer = null;
const isDistEvent = (file) =>
	typeof file === "string" &&
	(file === "dist" || file.startsWith("dist\\") || file.startsWith("dist/"));

const watcher = watch(EXT_ROOT, { recursive: true }, (_event, file) => {
	if (isDistEvent(file)) return;
	clearTimeout(timer);
	timer = setTimeout(() => rebuild(`change: ${file ?? "?"}`), DEBOUNCE_MS);
});

function shutdown() {
	console.log("\n[serve] shutting down");
	clearTimeout(timer);
	watcher.close();
	server.close(() => process.exit(0));
	// Hard stop if connections linger.
	setTimeout(() => process.exit(0), 500).unref();
}
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);
