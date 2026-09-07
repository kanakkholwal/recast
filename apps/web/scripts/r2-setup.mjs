#!/usr/bin/env node
/* eslint-disable no-console */
/**
 * R2 bucket setup — applies the CORS configuration needed for direct
 * browser PUTs from the dashboard and signed downloads from the player.
 *
 * R2-specific: other storage providers configure CORS through their own
 * native surfaces (Cloudinary in the dashboard, GCS via `gsutil cors set`,
 * Azure via the portal or `az storage cors add`). This script exits
 * silently if STORAGE_PROVIDER is set to anything other than `r2`.
 *
 * Run: `pnpm r2:setup`
 *
 * Prereqs:
 *   1. Bucket already exists in the Cloudflare dashboard. R2's S3-compat
 *      surface doesn't reliably expose CreateBucket; the dashboard is
 *      the canonical surface.
 *   2. `.env` has R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY,
 *      R2_BUCKET set. The API token must have "Object Read & Write"
 *      scoped to that bucket.
 *
 * Idempotent — re-running overwrites the CORS doc, so it's safe to use
 * after editing `ALLOWED_ORIGINS` below to add a new env.
 */

import { AwsClient } from "aws4fetch";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Minimal .env reader so a setup script doesn't pull dotenv into runtime deps; no `${...}` expansion, which isn't needed.
function loadEnvFile() {
	const path = resolve(__dirname, "..", ".env");
	try {
		const raw = readFileSync(path, "utf8");
		for (const line of raw.split(/\r?\n/)) {
			const trimmed = line.trim();
			if (!trimmed || trimmed.startsWith("#")) continue;
			const eq = trimmed.indexOf("=");
			if (eq === -1) continue;
			const key = trimmed.slice(0, eq).trim();
			let value = trimmed.slice(eq + 1).trim();
			if (
				(value.startsWith('"') && value.endsWith('"')) ||
				(value.startsWith("'") && value.endsWith("'"))
			) {
				value = value.slice(1, -1);
			}
			if (!(key in process.env)) process.env[key] = value;
		}
	} catch (err) {
		if (err.code !== "ENOENT") throw err;
	}
}

loadEnvFile();

const {
	STORAGE_PROVIDER,
	R2_ACCOUNT_ID,
	R2_ACCESS_KEY_ID,
	R2_SECRET_ACCESS_KEY,
	R2_BUCKET,
	PUBLIC_APP_URL,
} = process.env;

const activeProvider = (STORAGE_PROVIDER ?? "r2").toLowerCase();
if (activeProvider !== "r2") {
	console.log(
		`→ STORAGE_PROVIDER=${activeProvider}; r2:setup is R2-specific. Configure CORS through the ${activeProvider} provider's native surface and skip this script.`,
	);
	process.exit(0);
}

if (!R2_ACCOUNT_ID || !R2_ACCESS_KEY_ID || !R2_SECRET_ACCESS_KEY || !R2_BUCKET) {
	console.error(
		"✗ Missing R2 env. Set R2_ACCOUNT_ID, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, R2_BUCKET in apps/web/.env.",
	);
	process.exit(1);
}

// Production and dev hosts are always allowed, plus PUBLIC_APP_URL in case the dev port differs; edit here for a new env.
const ALLOWED_ORIGINS = [
	"http://localhost:4420",
	"http://localhost:4421",
	"http://localhost:5173",
	"http://tauri.localhost",
	"tauri://localhost",
	"https://recast.li",
	"https://www.recast.li",
	"https://recast.nexonauts.com",
	"https://recast-web.vercel.app",
	PUBLIC_APP_URL,
]
	.filter(Boolean)
	.filter((v, i, a) => a.indexOf(v) === i);

const corsXml = `<?xml version="1.0" encoding="UTF-8"?>
<CORSConfiguration>
  <CORSRule>
${ALLOWED_ORIGINS.map((o) => `    <AllowedOrigin>${escapeXml(o)}</AllowedOrigin>`).join("\n")}
    <AllowedMethod>GET</AllowedMethod>
    <AllowedMethod>PUT</AllowedMethod>
    <AllowedMethod>HEAD</AllowedMethod>
    <AllowedHeader>*</AllowedHeader>
    <ExposeHeader>ETag</ExposeHeader>
    <ExposeHeader>Content-Length</ExposeHeader>
    <MaxAgeSeconds>3600</MaxAgeSeconds>
  </CORSRule>
</CORSConfiguration>`;

const client = new AwsClient({
	accessKeyId: R2_ACCESS_KEY_ID,
	secretAccessKey: R2_SECRET_ACCESS_KEY,
	service: "s3",
	region: "auto",
});

const endpoint = `https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com/${R2_BUCKET}?cors`;

console.log(`→ Applying CORS to ${R2_BUCKET}`);
console.log(`  Origins (${ALLOWED_ORIGINS.length}):`);
for (const o of ALLOWED_ORIGINS) console.log(`    • ${o}`);

const res = await client.fetch(
	new Request(endpoint, {
		method: "PUT",
		headers: { "Content-Type": "application/xml" },
		body: corsXml,
	}),
);

if (!res.ok) {
	const body = await res.text().catch(() => "");
	console.error(`✗ R2 PutBucketCors failed: ${res.status} ${res.statusText}`);
	if (body) console.error(body);
	process.exit(1);
}

console.log("✓ CORS applied. Direct browser uploads should now work.");

function escapeXml(s) {
	return s
		.replaceAll("&", "&amp;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;")
		.replaceAll('"', "&quot;")
		.replaceAll("'", "&apos;");
}
