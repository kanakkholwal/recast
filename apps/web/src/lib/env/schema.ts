import { z } from "zod";

/**
 * Single source of truth for every environment variable the web app reads.
 *
 * Two schemas — server and public — kept apart so client code can never pull
 * a secret by accident. `PUBLIC_*` vars travel to the browser (SvelteKit
 * enforces this at import-time via `$env/static/public`); everything else is
 * server-only and is read through `$env/dynamic/private`.
 *
 * To add a new var:
 *   1. Add it here with the strictest schema you can stand (`url()`, `min()`,
 *      `enum()` — not bare `string()`).
 *   2. If it's optional, give it a `.default()` or mark `.optional()`.
 *   3. Add a matching line to `.env.example` so the next dev knows it exists.
 *   4. Read it from `serverEnv` / `publicEnv` — never from `$env/...` directly.
 */

// `optional()` first so a missing var (input === undefined) passes through
// the pipe without tripping the inner `z.string()` check. Then trim and
// collapse empty/whitespace-only values to `undefined` so the downstream
// `.optional()` / required checks see a clean state.
const trimmed = z
	.string()
	.optional()
	.transform((v) => (v ?? "").trim())
	.transform((v) => (v.length === 0 ? undefined : v));

// Treats `""` (the default in .env.example for blank-out fields) the same as
// missing, so empty strings don't satisfy `.optional()` and accidentally enable
// half-configured providers downstream.
const optionalSecret = trimmed.pipe(z.string().min(1).optional());

const optionalUrl = trimmed.pipe(z.url().optional());

// Comma-separated list → string[]. Drops blanks so a trailing comma or stray
// whitespace doesn't sneak an empty string into better-auth's allow-list.
const optionalCsv = trimmed.pipe(
	z
		.string()
		.default(`https://recast.li,https://www.recast.li,https://recast.nexonauts.com`)
		.optional()
		.transform((v) =>
			v
				? v
						.split(",")
						.map((s) => s.trim())
						.filter((s) => s.length > 0)
				: [],
		),
);

export const serverEnvSchema = z
	.object({
		//  Database
		DATABASE_URL: trimmed.pipe(
			z.string().min(1, "DATABASE_URL is required — set a Postgres connection string"),
		),

		//  Better Auth
		BETTER_AUTH_SECRET: trimmed.pipe(
			z.string().min(32, "BETTER_AUTH_SECRET must be ≥32 chars — `openssl rand -base64 32`"),
		),
		BETTER_AUTH_URL: optionalUrl,
		// Extra origins better-auth should accept beyond BETTER_AUTH_URL /
		// PUBLIC_APP_URL. Comma-separated; supports wildcards (e.g.
		// `https://*.vercel.app`). The known production hosts are merged in
		// auth/server.ts, so leave this blank unless you're adding a new one.
		TRUSTED_ORIGINS: optionalCsv,

		//  OAuth (optional pairs — see superRefine below)
		GITHUB_CLIENT_ID: optionalSecret,
		GITHUB_CLIENT_SECRET: optionalSecret,
		GOOGLE_CLIENT_ID: optionalSecret,
		GOOGLE_CLIENT_SECRET: optionalSecret,

		//  Polar (billing — all-or-nothing trio, see superRefine below)
		POLAR_SERVER: z.enum(["sandbox", "production"]).default("sandbox"),
		POLAR_ACCESS_TOKEN: optionalSecret,
		POLAR_WEBHOOK_SECRET: optionalSecret,
		POLAR_PRODUCT_ID_PRO: optionalSecret,

		//  Email
		RESEND_API_KEY: optionalSecret,
		EMAIL_FROM: trimmed
			.pipe(z.string().min(1).optional())
			.transform((v) => v ?? "Recast <hello@recast.nexonauts.com>"),

		//  Cloud storage provider switch
		// "r2" (default) | "s3" | "cloudinary" | "azure" | "gcs". Each
		// provider's credentials live in its own block below; only the
		// active provider's block needs to be set.
		STORAGE_PROVIDER: trimmed
			.pipe(z.enum(["r2", "s3", "cloudinary", "azure", "gcs"]).optional())
			.optional(),

		//  Cloudflare R2 (all-or-nothing quartet, see superRefine below)
		R2_ACCOUNT_ID: optionalSecret,
		R2_ACCESS_KEY_ID: optionalSecret,
		R2_SECRET_ACCESS_KEY: optionalSecret,
		R2_BUCKET: optionalSecret,
		R2_PUBLIC_URL: optionalUrl,

		//  AWS S3 (or S3-compat: MinIO, Wasabi, B2, DO Spaces)
		S3_REGION: optionalSecret,
		S3_BUCKET: optionalSecret,
		S3_ACCESS_KEY_ID: optionalSecret,
		S3_SECRET_ACCESS_KEY: optionalSecret,
		S3_ENDPOINT: optionalUrl, // for S3-compat hosts; leave blank for AWS
		S3_PUBLIC_URL: optionalUrl,

		//  Cloudinary
		CLOUDINARY_CLOUD_NAME: optionalSecret,
		CLOUDINARY_API_KEY: optionalSecret,
		CLOUDINARY_API_SECRET: optionalSecret,

		//  Azure Blob Storage
		AZURE_STORAGE_ACCOUNT: optionalSecret,
		AZURE_STORAGE_KEY: optionalSecret,
		AZURE_BLOB_CONTAINER: optionalSecret,
		AZURE_PUBLIC_URL: optionalUrl,

		//  Google Cloud Storage
		GCS_BUCKET: optionalSecret,
		// Paste the entire service-account JSON as a single line.
		GCS_SERVICE_ACCOUNT_JSON: optionalSecret,
		GCS_PUBLIC_URL: optionalUrl,

		//  Cron secret (gate /api/cron/* endpoints)
		CRON_SECRET: optionalSecret,

		//  Runtime mode (set by hosts like Vercel/Node)
		NODE_ENV: z.enum(["development", "test", "production"]).default("development"),
	})
	.superRefine((env, ctx) => {
		const pair = (a: string, b: string, aVal: string | undefined, bVal: string | undefined) => {
			if (Boolean(aVal) !== Boolean(bVal)) {
				ctx.addIssue({
					code: "custom",
					path: [aVal ? b : a],
					message: `${a} and ${b} must be set together`,
				});
			}
		};
		pair(
			"GITHUB_CLIENT_ID",
			"GITHUB_CLIENT_SECRET",
			env.GITHUB_CLIENT_ID,
			env.GITHUB_CLIENT_SECRET,
		);
		pair(
			"GOOGLE_CLIENT_ID",
			"GOOGLE_CLIENT_SECRET",
			env.GOOGLE_CLIENT_ID,
			env.GOOGLE_CLIENT_SECRET,
		);

		const polarVars = [env.POLAR_ACCESS_TOKEN, env.POLAR_WEBHOOK_SECRET, env.POLAR_PRODUCT_ID_PRO];
		const polarSet = polarVars.filter(Boolean).length;
		if (polarSet !== 0 && polarSet !== polarVars.length) {
			ctx.addIssue({
				code: "custom",
				path: ["POLAR_ACCESS_TOKEN"],
				message:
					"POLAR_ACCESS_TOKEN, POLAR_WEBHOOK_SECRET, and POLAR_PRODUCT_ID_PRO must all be set together (or all left blank to disable billing).",
			});
		}

		// Storage provider validation — only police the *active* provider.
		// When STORAGE_PROVIDER is set explicitly, require its full set of
		// vars. When unset, applied per-provider as "all or nothing" so a
		// half-filled R2 block (typical when copying from .env.example)
		// still produces a useful error, while having stray vars from a
		// provider you switched away from doesn't.
		// Azure accepts either a bare account name + standalone key, OR a full
		// connection string pasted into AZURE_STORAGE_ACCOUNT (which carries its
		// own AccountKey=). Detect the connection-string form with the same regex
		// `resolveAzureCredentials()` / `isStorageConfigured()` use, and only
		// require the standalone AZURE_STORAGE_KEY in the bare-name case.
		const azureIsConnString = /(^|;)\s*AccountName=/i.test(env.AZURE_STORAGE_ACCOUNT ?? "");
		const azureVars: (readonly [string, unknown])[] = [
			["AZURE_STORAGE_ACCOUNT", env.AZURE_STORAGE_ACCOUNT],
			...(azureIsConnString ? [] : ([["AZURE_STORAGE_KEY", env.AZURE_STORAGE_KEY]] as const)),
			["AZURE_BLOB_CONTAINER", env.AZURE_BLOB_CONTAINER],
		];

		const providerVarSpecs = {
			r2: {
				vars: [
					["R2_ACCOUNT_ID", env.R2_ACCOUNT_ID],
					["R2_ACCESS_KEY_ID", env.R2_ACCESS_KEY_ID],
					["R2_SECRET_ACCESS_KEY", env.R2_SECRET_ACCESS_KEY],
					["R2_BUCKET", env.R2_BUCKET],
				] as const,
			},
			s3: {
				vars: [
					["S3_REGION", env.S3_REGION],
					["S3_BUCKET", env.S3_BUCKET],
					["S3_ACCESS_KEY_ID", env.S3_ACCESS_KEY_ID],
					["S3_SECRET_ACCESS_KEY", env.S3_SECRET_ACCESS_KEY],
				] as const,
			},
			cloudinary: {
				vars: [
					["CLOUDINARY_CLOUD_NAME", env.CLOUDINARY_CLOUD_NAME],
					["CLOUDINARY_API_KEY", env.CLOUDINARY_API_KEY],
					["CLOUDINARY_API_SECRET", env.CLOUDINARY_API_SECRET],
				] as const,
			},
			azure: {
				vars: azureVars,
			},
			gcs: {
				vars: [
					["GCS_BUCKET", env.GCS_BUCKET],
					["GCS_SERVICE_ACCOUNT_JSON", env.GCS_SERVICE_ACCOUNT_JSON],
				] as const,
			},
		};

		if (env.STORAGE_PROVIDER) {
			// Explicit provider: every var for it must be set; ignore the rest.
			const spec = providerVarSpecs[env.STORAGE_PROVIDER];
			const missing = spec.vars.filter(([, value]) => !value).map(([name]) => name);
			if (missing.length > 0) {
				ctx.addIssue({
					code: "custom",
					path: [spec.vars[0][0]],
					message: `STORAGE_PROVIDER="${env.STORAGE_PROVIDER}" requires: ${missing.join(", ")}.`,
				});
			}
		} else {
			// No explicit provider: enforce all-or-nothing per provider so a
			// half-filled block (typo, partial copy from .env.example) still
			// gets caught, without forcing you to set a provider at all.
			for (const [name, spec] of Object.entries(providerVarSpecs)) {
				const setCount = spec.vars.filter(([, v]) => Boolean(v)).length;
				if (setCount !== 0 && setCount !== spec.vars.length) {
					const missing = spec.vars.filter(([, v]) => !v).map(([n]) => n);
					ctx.addIssue({
						code: "custom",
						path: [spec.vars[0][0]],
						message: `Partial ${name} config — set STORAGE_PROVIDER=${name} and fill ${missing.join(", ")}, or clear the block to disable.`,
					});
				}
			}
		}
	});

export type ServerEnv = z.infer<typeof serverEnvSchema>;

export const publicEnvSchema = z.object({
	PUBLIC_APP_URL: z
		.string()
		.transform((v) => v.trim())
		.pipe(z.url())
		.default("http://localhost:5173"),
	PUBLIC_APP_NAME: z
		.string()
		.transform((v) => v.trim())
		.pipe(z.string().min(1))
		.default("Recast"),

	//  Analytics (PostHog) — optional; when PUBLIC_POSTHOG_KEY is blank the
	//  analytics client is a total no-op (mirrors isStorageConfigured()). EU
	//  Cloud host by default to keep data in-region for GDPR.
	PUBLIC_POSTHOG_KEY: trimmed.pipe(z.string().min(1).optional()),
	PUBLIC_POSTHOG_HOST: trimmed
		.pipe(z.url().optional())
		.transform((v) => v ?? "https://eu.i.posthog.com"),
	PUBLIC_POSTHOG_UI_HOST: optionalUrl,
});

export type PublicEnv = z.infer<typeof publicEnvSchema>;
