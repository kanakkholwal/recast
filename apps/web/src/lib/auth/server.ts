import { dev } from "$app/environment";
import {
	checkout,
	polar,
	portal,
	webhooks,
} from "@polar-sh/better-auth";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { admin, magicLink } from "better-auth/plugins";
import { eq } from "drizzle-orm";
import { sendEmail } from "$lib/auth/email";
import { polarProductIdFor } from "$lib/billing/plans";
import { tryGetPolarClient } from "$lib/billing/polar";
import { downgradeToFree, upsertSubscription } from "$lib/billing/sync";
import { getDb } from "$lib/db";
import { user as userTable } from "$lib/db/schema";
import * as schema from "$lib/db/schema";
import { serverEnv } from "$lib/env/server";
import { publicEnv } from "$lib/env/public";

/**
 * Better Auth instance — singleton, lazy-built on first request so the
 * Drizzle adapter doesn't open a Postgres connection at module load time
 * (matters for `pnpm build` in environments where DATABASE_URL is set at
 * runtime, not build time).
 *
 * Required env: DATABASE_URL, BETTER_AUTH_SECRET.
 * Optional env: BETTER_AUTH_URL, GITHUB_*, GOOGLE_*, POLAR_*, RESEND_API_KEY.
 */

function createAuth() {
	const env = serverEnv();

	return betterAuth({
		secret: env.BETTER_AUTH_SECRET,
		baseURL: env.BETTER_AUTH_URL ?? publicEnv().PUBLIC_APP_URL,
		database: drizzleAdapter(getDb(), { provider: "pg", schema }),
		// `status` is an app-owned column, separate from the plugin-owned
		// `role`. Surfaces on session.user so the dashboard load can read it.
		user: {
			additionalFields: {
				status: { type: "string", defaultValue: "active", required: false },
			},
		},
		emailAndPassword: {
			enabled: true,
			// In production, public sign-up is closed — the waitlist endpoint
			// creates user rows directly. Dev keeps signup open for iteration.
			disableSignUp: !dev,
			requireEmailVerification: false,
			sendResetPassword: async ({ user, url }) => {
				if (await isOnWaitlist(user.email)) return;
				await sendEmail({
					to: user.email,
					subject: "Reset your Recast password",
					text:
						`Hi${user.name ? ` ${user.name}` : ""},\n\n` +
						`Click the link below to set a new password:\n${url}\n\n` +
						`If you didn't ask for this, you can ignore the email.`,
				});
			},
		},
		socialProviders: buildSocialProviders(),
		plugins: buildPlugins(),
	});
}

type AuthInstance = ReturnType<typeof createAuth>;

let cached: AuthInstance | null = null;

export function getAuth(): AuthInstance {
	if (cached) return cached;
	cached = createAuth();
	return cached;
}

function buildSocialProviders() {
	const providers: Record<string, { clientId: string; clientSecret: string }> = {};
	// Social sign-in is dev-only for now — production gates this at the UI
	// level (SocialButtons.svelte) and here so misconfigured client IDs
	// can't accidentally enable a path.
	if (!dev) return providers;
	const env = serverEnv();
	if (env.GITHUB_CLIENT_ID && env.GITHUB_CLIENT_SECRET) {
		providers.github = {
			clientId: env.GITHUB_CLIENT_ID,
			clientSecret: env.GITHUB_CLIENT_SECRET,
		};
	}
	if (env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET) {
		providers.google = {
			clientId: env.GOOGLE_CLIENT_ID,
			clientSecret: env.GOOGLE_CLIENT_SECRET,
		};
	}
	return providers;
}

function buildPlugins() {
	// Admin plugin — owns `role`, `banned`, `banReason`, `banExpires` on
	// user and `impersonatedBy` on session. Endpoints live under
	// /api/auth/admin/* with built-in 403 for non-admins.
	const adminPlugin = admin({
		defaultRole: "user",
		adminRoles: ["admin"],
		impersonationSessionDuration: 60 * 60, // 1h
	});

	const linkPlugin = magicLink({
		// Existing-users-only — waitlist sign-up is the only way to get a row.
		disableSignUp: true,
		expiresIn: 60 * 10,
		sendMagicLink: async ({ email, url }) => {
			if (await isOnWaitlist(email)) return;
			await sendEmail({
				to: email,
				subject: "Your Recast sign-in link",
				text:
					`Click the link below to sign in to Recast:\n\n${url}\n\n` +
					`The link expires in 10 minutes. If you didn't ask for it, ignore this email.`,
			});
		},
	});

	const polarClient = tryGetPolarClient();
	const proProductId = polarProductIdFor("pro");
	const webhookSecret = serverEnv().POLAR_WEBHOOK_SECRET;

	const polarPlugins =
		polarClient && proProductId && webhookSecret
			? [
					polar({
						client: polarClient,
						createCustomerOnSignUp: true,
						use: [
							checkout({
								products: [{ productId: proProductId, slug: "pro" }],
								successUrl: "/dashboard?upgraded=1",
								authenticatedUsersOnly: true,
							}),
							portal(),
							webhooks({
								secret: webhookSecret,
								onSubscriptionActive: async (payload) =>
									handleSubscriptionEvent(payload),
								onSubscriptionUpdated: async (payload) =>
									handleSubscriptionEvent(payload),
								onSubscriptionCanceled: async (payload) => {
									const userId = extractUserId(payload);
									if (userId) await downgradeToFree(userId);
								},
								onSubscriptionRevoked: async (payload) => {
									const userId = extractUserId(payload);
									if (userId) await downgradeToFree(userId);
								},
							}),
						],
					}),
				]
			: [];

	return [adminPlugin, linkPlugin, ...polarPlugins];
}

/**
 * Returns true if the user with this email exists and is still pending
 * waitlist approval. Magic link + password reset both short-circuit on
 * this so pending users can't slip in via either path before being
 * activated (admin flips status: pending → active in /admin/waitlist).
 */
async function isOnWaitlist(email: string): Promise<boolean> {
	const db = getDb();
	const rows = await db
		.select({ status: userTable.status })
		.from(userTable)
		.where(eq(userTable.email, email))
		.limit(1);
	return rows[0]?.status === "pending";
}

async function handleSubscriptionEvent(payload: unknown): Promise<void> {
	const data = (payload as { data?: Record<string, unknown> })?.data ?? {};
	const userId = extractUserId(payload);
	const polarCustomerId = String(data.customerId ?? data.customer_id ?? "");
	const polarSubscriptionId = String(data.id ?? "");
	const status = String(data.status ?? "active") as Parameters<
		typeof upsertSubscription
	>[0]["status"];
	const periodEndRaw = (data.currentPeriodEnd ?? data.current_period_end) as
		| string
		| number
		| null
		| undefined;
	const currentPeriodEnd = periodEndRaw ? new Date(periodEndRaw) : null;
	const cancelAtPeriodEnd = Boolean(
		data.cancelAtPeriodEnd ?? data.cancel_at_period_end ?? false,
	);

	if (!userId || !polarSubscriptionId) return;

	await upsertSubscription({
		userId,
		polarCustomerId,
		polarSubscriptionId,
		plan: "pro",
		status,
		currentPeriodEnd,
		cancelAtPeriodEnd,
	});
}

function extractUserId(payload: unknown): string | null {
	const data = (payload as { data?: Record<string, unknown> })?.data ?? {};
	const v =
		data.customerExternalId ??
		data.customer_external_id ??
		((data.customer as Record<string, unknown> | undefined)?.externalId as
			| string
			| undefined);
	return typeof v === "string" && v.length > 0 ? v : null;
}
