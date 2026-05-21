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

const trimmed = z
	.string()
	.transform((v) => v.trim())
	.transform((v) => (v.length === 0 ? undefined : v));

// Treats `""` (the default in .env.example for blank-out fields) the same as
// missing, so empty strings don't satisfy `.optional()` and accidentally enable
// half-configured providers downstream.
const optionalSecret = trimmed.pipe(z.string().min(1).optional());

const optionalUrl = trimmed.pipe(z.url().optional());

export const serverEnvSchema = z
	.object({
		// ── Database ────────────────────────────────────────────────────────
		DATABASE_URL: trimmed.pipe(
			z
				.string()
				.min(1, "DATABASE_URL is required — set a Postgres connection string"),
		),

		// ── Better Auth ─────────────────────────────────────────────────────
		BETTER_AUTH_SECRET: trimmed.pipe(
			z
				.string()
				.min(
					32,
					"BETTER_AUTH_SECRET must be ≥32 chars — `openssl rand -base64 32`",
				),
		),
		BETTER_AUTH_URL: optionalUrl,

		// ── OAuth (optional pairs — see superRefine below) ──────────────────
		GITHUB_CLIENT_ID: optionalSecret,
		GITHUB_CLIENT_SECRET: optionalSecret,
		GOOGLE_CLIENT_ID: optionalSecret,
		GOOGLE_CLIENT_SECRET: optionalSecret,

		// ── Polar (billing — all-or-nothing trio, see superRefine below) ────
		POLAR_SERVER: z.enum(["sandbox", "production"]).default("sandbox"),
		POLAR_ACCESS_TOKEN: optionalSecret,
		POLAR_WEBHOOK_SECRET: optionalSecret,
		POLAR_PRODUCT_ID_PRO: optionalSecret,

		// ── Email ───────────────────────────────────────────────────────────
		RESEND_API_KEY: optionalSecret,
		EMAIL_FROM: trimmed
			.pipe(z.string().min(1).optional())
			.transform((v) => v ?? "Recast <hello@recast.nexonauts.com>"),

		// ── Runtime mode (set by hosts like Vercel/Node) ─────────────────────
		NODE_ENV: z.enum(["development", "test", "production"]).default("development"),
	})
	.superRefine((env, ctx) => {
		const pair = (
			a: string,
			b: string,
			aVal: string | undefined,
			bVal: string | undefined,
		) => {
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

		const polarVars = [
			env.POLAR_ACCESS_TOKEN,
			env.POLAR_WEBHOOK_SECRET,
			env.POLAR_PRODUCT_ID_PRO,
		];
		const polarSet = polarVars.filter(Boolean).length;
		if (polarSet !== 0 && polarSet !== polarVars.length) {
			ctx.addIssue({
				code: "custom",
				path: ["POLAR_ACCESS_TOKEN"],
				message:
					"POLAR_ACCESS_TOKEN, POLAR_WEBHOOK_SECRET, and POLAR_PRODUCT_ID_PRO must all be set together (or all left blank to disable billing).",
			});
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
});

export type PublicEnv = z.infer<typeof publicEnvSchema>;
