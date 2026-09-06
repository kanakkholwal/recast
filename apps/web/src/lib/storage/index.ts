import { Files, type SignedUpload } from "files-sdk";
import { r2 } from "files-sdk/r2";
import { s3 } from "files-sdk/s3";
import { serverEnv } from "$lib/env/server";

/**
 * Provider-agnostic blob storage wrapper.
 *
 * Backed by [`files-sdk`](https://files-sdk.dev). The active provider is
 * chosen by `STORAGE_PROVIDER` env (`r2` | `s3` | `cloudinary` | `azure`
 * | `gcs`). Each provider's credentials live in its own env vars; the
 * matching adapter is dynamically required so unused providers don't
 * even need their peer dep installed.
 *
 * Public API (called from upload / share / expire endpoints):
 *   - `isStorageConfigured()`
 *   - `recastObjectKey(workspaceId, recastId)` / `posterObjectKey(...)`
 *   - `signUploadUrl({ key, contentType, expiresInSeconds })` →
 *       `{ method: "PUT" | "POST", url, headers?, fields? }`
 *   - `signDownloadUrl({ key, expiresInSeconds })` → `string`
 *   - `statObject(key)` → `{ contentLength, contentType, etag } | null`
 *   - `deleteObject(key)`
 *   - `publicObjectUrl(key)` → `string | null`
 *
 * Swapping providers is a one-env-var change — no consumer code changes.
 * The R2 path is the v1 default; the others are wired for portability.
 */

export type StorageProvider = "r2" | "s3" | "cloudinary" | "azure" | "gcs";

let cached: { provider: StorageProvider; files: Files; publicBaseUrl: string | null } | null = null;

function activeProvider(): StorageProvider {
	const env = serverEnv();
	// `STORAGE_PROVIDER` is optional and falls back to `r2`, so R2-only deployments keep working.
	const raw = (env.STORAGE_PROVIDER ?? "r2").toLowerCase();
	switch (raw) {
		case "r2":
		case "s3":
		case "cloudinary":
		case "azure":
		case "gcs":
			return raw;
		default:
			throw new Error(
				`Unknown STORAGE_PROVIDER=${raw}; expected one of r2 | s3 | cloudinary | azure | gcs`,
			);
	}
}

// Credentials are only known-present when isStorageConfigured() passed; name the missing var rather than failing downstream in the adapter.
function required(name: string, value: string | undefined | null): string {
	if (!value) throw new Error(`STORAGE_PROVIDER=${activeProvider()} requires ${name} to be set`);
	return value;
}

export function isStorageConfigured(): boolean {
	const env = serverEnv();
	const provider = (env.STORAGE_PROVIDER ?? "r2").toLowerCase();
	switch (provider) {
		case "r2":
			return Boolean(
				env.R2_ACCOUNT_ID && env.R2_ACCESS_KEY_ID && env.R2_SECRET_ACCESS_KEY && env.R2_BUCKET,
			);
		case "s3":
			return Boolean(
				env.S3_REGION && env.S3_BUCKET && env.S3_ACCESS_KEY_ID && env.S3_SECRET_ACCESS_KEY,
			);
		case "cloudinary":
			return Boolean(
				env.CLOUDINARY_CLOUD_NAME && env.CLOUDINARY_API_KEY && env.CLOUDINARY_API_SECRET,
			);
		case "azure": {
			const account = env.AZURE_STORAGE_ACCOUNT ?? "";
			// Self-sufficient only with BOTH AccountName= and AccountKey=; a SAS string still needs AZURE_STORAGE_KEY.
			const hasInlineKey =
				/(^|;)\s*AccountName=/i.test(account) && /(^|;)\s*AccountKey=/i.test(account);
			return Boolean(
				account && env.AZURE_BLOB_CONTAINER && (hasInlineKey || env.AZURE_STORAGE_KEY),
			);
		}
		case "gcs":
			return Boolean(env.GCS_BUCKET && env.GCS_SERVICE_ACCOUNT_JSON);
		default:
			return false;
	}
}

async function buildFiles(): Promise<{
	files: Files;
	publicBaseUrl: string | null;
}> {
	const provider = activeProvider();
	const env = serverEnv();

	switch (provider) {
		case "r2": {
			return {
				files: new Files({
					adapter: r2({
						bucket: required("R2_BUCKET", env.R2_BUCKET),
						accountId: required("R2_ACCOUNT_ID", env.R2_ACCOUNT_ID),
						accessKeyId: required("R2_ACCESS_KEY_ID", env.R2_ACCESS_KEY_ID),
						secretAccessKey: required("R2_SECRET_ACCESS_KEY", env.R2_SECRET_ACCESS_KEY),
						publicBaseUrl: env.R2_PUBLIC_URL ?? undefined,
					}),
				}),
				publicBaseUrl: env.R2_PUBLIC_URL ?? null,
			};
		}

		case "s3": {
			return {
				files: new Files({
					adapter: s3({
						bucket: required("S3_BUCKET", env.S3_BUCKET),
						region: required("S3_REGION", env.S3_REGION),
						credentials: {
							accessKeyId: required("S3_ACCESS_KEY_ID", env.S3_ACCESS_KEY_ID),
							secretAccessKey: required("S3_SECRET_ACCESS_KEY", env.S3_SECRET_ACCESS_KEY),
						},
						endpoint: env.S3_ENDPOINT ?? undefined,
					}),
				}),
				publicBaseUrl: env.S3_PUBLIC_URL ?? null,
			};
		}

		case "cloudinary": {
			// Lazy import to keep `cloudinary` out of the bundle unless used.
			const { cloudinary } = await import("files-sdk/cloudinary");
			return {
				files: new Files({
					adapter: cloudinary({
						cloudName: required("CLOUDINARY_CLOUD_NAME", env.CLOUDINARY_CLOUD_NAME),
						apiKey: required("CLOUDINARY_API_KEY", env.CLOUDINARY_API_KEY),
						apiSecret: required("CLOUDINARY_API_SECRET", env.CLOUDINARY_API_SECRET),
						// Cloudinary auto-detects video from the filename, but pin it so files that bypass transforms still PUT and GET.
						resourceType: "video",
					}),
				}),
				publicBaseUrl: null,
			};
		}

		case "azure": {
			const { azure } = await import("files-sdk/azure");
			// The portal hands you a connection string, so accept one in AZURE_STORAGE_ACCOUNT and parse the name and key out of it.
			const { accountName, accountKey } = resolveAzureCredentials(
				required("AZURE_STORAGE_ACCOUNT", env.AZURE_STORAGE_ACCOUNT),
				env.AZURE_STORAGE_KEY,
			);
			return {
				files: new Files({
					adapter: azure({
						accountName,
						accountKey,
						container: required("AZURE_BLOB_CONTAINER", env.AZURE_BLOB_CONTAINER),
					}),
				}),
				publicBaseUrl: env.AZURE_PUBLIC_URL ?? null,
			};
		}

		case "gcs": {
			const { gcs } = await import("files-sdk/gcs");
			return {
				files: new Files({
					adapter: gcs({
						bucket: required("GCS_BUCKET", env.GCS_BUCKET),
						// Service-account JSON pasted into the env var as one line; parsed here so callers don't need the shape.
						credentials: JSON.parse(
							required("GCS_SERVICE_ACCOUNT_JSON", env.GCS_SERVICE_ACCOUNT_JSON),
						),
					}),
				}),
				publicBaseUrl: env.GCS_PUBLIC_URL ?? null,
			};
		}
	}
}

async function get(): Promise<{
	provider: StorageProvider;
	files: Files;
	publicBaseUrl: string | null;
}> {
	if (cached) return cached;
	if (!isStorageConfigured()) {
		throw new Error(
			`Storage provider "${activeProvider()}" is not configured. Set the matching env vars in apps/web/.env.`,
		);
	}
	const built = await buildFiles();
	cached = { provider: activeProvider(), ...built };
	return cached;
}

/**
 * Object key layout: `workspace/{workspaceId}/{recastId}.mp4`. Stable
 * across providers — keys are opaque strings everywhere.
 */
export function recastObjectKey(workspaceId: string, recastId: string): string {
	return `workspace/${workspaceId}/${recastId}.mp4`;
}

/**
 * Poster key. The original upload writes the unversioned key
 * (`{recastId}.poster.webp`); a *replacement* passes a short `version` token so
 * it lands on a NEW key (`{recastId}.poster.{version}.webp`). The new key both
 * busts any CDN cache of the old poster and gives the replace flow an old key
 * to delete — overwriting in place would do neither.
 */
export function posterObjectKey(workspaceId: string, recastId: string, version?: string): string {
	const v = version ? `.${version.replace(/[^a-z0-9]/gi, "")}` : "";
	return `workspace/${workspaceId}/${recastId}.poster${v}.webp`;
}

/**
 * Captions track key: `workspace/{workspaceId}/{recastId}.captions.vtt`.
 * One WebVTT sidecar per recast, loaded by the player as a `<track>`.
 */
export function captionsObjectKey(workspaceId: string, recastId: string): string {
	return `workspace/${workspaceId}/${recastId}.captions.vtt`;
}

/**
 * Reverse a stored `posterUrl`/`videoUrl` back into the object key we can pass
 * to `deleteObject`. Bare keys (the common case) return as-is; an absolute URL
 * is matched against the active provider's public base URL and its path
 * extracted. Returns null when the value is an external/legacy URL we don't
 * own — the caller then skips the delete rather than risk touching a foreign
 * object.
 */
export function objectKeyFromStored(value: string | null | undefined): string | null {
	const v = (value ?? "").trim();
	if (!v) return null;
	if (!/^https?:\/\//.test(v)) return v; // already a bare key
	const base = cached?.publicBaseUrl;
	if (!base) return null;
	const prefix = `${base.replace(/\/$/, "")}/`;
	if (!v.startsWith(prefix)) return null;
	try {
		return decodeURI(v.slice(prefix.length));
	} catch {
		return v.slice(prefix.length);
	}
}

/**
 * Sign a direct-from-client upload. Returns either a PUT envelope (the
 * client `fetch(url, { method: "PUT", body: file, headers })`) or a POST
 * envelope (multipart form built from `fields` + the file blob).
 *
 * Cloudinary always returns POST; S3 / R2 / Azure / GCS return PUT
 * unless `maxSize` is set (then S3-family adapters switch to POST with
 * a content-length-range policy). Pass `maxSize` when you want
 * server-enforced size caps; we omit it for now since the quota
 * pre-check + the /complete-side HEAD recheck cover that.
 */
export async function signUploadUrl(opts: {
	key: string;
	contentType?: string;
	expiresInSeconds?: number;
}): Promise<SignedUpload> {
	const { files, provider } = await get();
	// Azure SAS never signs Content-Type and files-sdk rejects the option, so passing it fails every signed upload.
	const contentType = provider === "azure" ? undefined : (opts.contentType ?? "video/mp4");
	return files.signedUploadUrl(opts.key, {
		expiresIn: opts.expiresInSeconds ?? 15 * 60,
		...(contentType ? { contentType } : {}),
	});
}

/**
 * Pre-signed GET URL for hosted playback. Returned to the player after
 * the visibility / password check passes. 1h TTL by default.
 */
export async function signDownloadUrl(opts: {
	key: string;
	expiresInSeconds?: number;
}): Promise<string> {
	const { files } = await get();
	return files.url(opts.key, { expiresIn: opts.expiresInSeconds ?? 60 * 60 });
}

/**
 * HEAD the object to verify the upload actually landed and report its
 * server-side size. Used by /api/uploads/complete to reject calls that
 * never actually PUT anything. Returns `null` on 404.
 */
export async function statObject(key: string): Promise<{
	contentLength: number;
	contentType: string | null;
	etag: string | null;
} | null> {
	const { files } = await get();
	try {
		const head = await files.head(key);
		return {
			contentLength: head.size,
			contentType: head.type ?? null,
			etag: head.etag ?? null,
		};
	} catch (err) {
		// files-sdk throws FilesError with code 'NotFound' when the object is missing; other errors propagate.
		if (isNotFoundError(err)) return null;
		throw err;
	}
}

/** Delete an object. Idempotent — missing objects do not throw. */
export async function deleteObject(key: string): Promise<void> {
	const { files } = await get();
	try {
		await files.delete(key);
	} catch (err) {
		if (isNotFoundError(err)) return;
		throw err;
	}
}

/**
 * Resolve a stored `videoUrl`/`posterUrl` (or any object key) into something a
 * browser can actually fetch. Uploads persist the *bare key* (e.g.
 * `workspace/{ws}/{recast}.mp4`); for providers without a public base URL that
 * key isn't directly playable, so we mint a short-lived signed GET — the same
 * thing the share page does.
 *
 * `objectKeyFromStored` normalizes the input first: a bare key stays a key, and
 * an absolute URL that points at *our own* public base is turned back into its
 * key so it gets signed too. This heals rows that persisted a public-CDN URL
 * (e.g. posters written as `publicObjectUrl(key)`) when that CDN isn't actually
 * serving them — they now sign the underlying object like the video does.
 * Truly external URLs (no matching base) pass through unchanged, and a signing
 * failure degrades to the raw value rather than throwing the whole page load.
 */
export async function resolvePlaybackUrl(
	keyOrUrl: string | null | undefined,
	expiresInSeconds?: number,
): Promise<string> {
	const value = keyOrUrl ?? "";
	if (!value) return value;
	if (!isStorageConfigured()) return value;
	const key = objectKeyFromStored(value);
	if (!key) return value; // external absolute URL we don't own — leave as-is
	try {
		return await signDownloadUrl({ key, expiresInSeconds });
	} catch (err) {
		console.error("[storage] resolvePlaybackUrl sign failed", err);
		return value;
	}
}

/**
 * Public URL for the object when the provider exposes one (R2 with
 * custom domain, public S3, Cloudinary `secure_url`, etc.). Returns
 * null when only signed reads are possible.
 */
export function publicObjectUrl(key: string): string | null {
	// Only safe with a warm client cache; otherwise return null and let the caller fall back to signed GETs.
	if (!cached) return null;
	const base = cached.publicBaseUrl;
	if (!base) return null;
	return `${base.replace(/\/$/, "")}/${encodeURI(key)}`;
}

/**
 * Resolve Azure credentials from env, tolerating a full connection string
 * pasted into the "account" slot (the portal hands you a connection string,
 * not a bare account name — an easy mistake). Returns the account name + key
 * the adapter needs. For a plain account name, the separate `keyRaw` is used.
 */
function resolveAzureCredentials(
	accountRaw: string,
	keyRaw: string | undefined,
): { accountName: string; accountKey: string } {
	// A connection string is Key=Value pairs; the base64 AccountKey contains '=' but never ';', so this split is safe.
	if (/(^|;)\s*AccountName=/i.test(accountRaw)) {
		const parts: Record<string, string> = {};
		for (const seg of accountRaw.split(";")) {
			const eq = seg.indexOf("=");
			if (eq <= 0) continue;
			parts[seg.slice(0, eq).trim().toLowerCase()] = seg.slice(eq + 1).trim();
		}
		return {
			accountName: parts.accountname ?? accountRaw,
			accountKey: parts.accountkey ?? keyRaw ?? "",
		};
	}
	return { accountName: accountRaw, accountKey: keyRaw ?? "" };
}

function isNotFoundError(err: unknown): boolean {
	if (!err || typeof err !== "object") return false;
	const e = err as { code?: unknown; name?: unknown; message?: unknown };
	if (e.code === "NotFound") return true;
	if (
		e.name === "FilesError" &&
		typeof e.message === "string" &&
		e.message.toLowerCase().includes("not found")
	) {
		return true;
	}
	return false;
}
