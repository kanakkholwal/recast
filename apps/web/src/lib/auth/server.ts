import { env } from "$env/dynamic/private";
import {
	checkout,
	polar,
	portal,
	webhooks,
} from "@polar-sh/better-auth";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { tryGetPolarClient } from "$lib/billing/polar";
import { polarProductIdFor } from "$lib/billing/plans";
import { downgradeToFree, upsertSubscription } from "$lib/billing/sync";
import { getDb } from "$lib/db";
import * as schema from "$lib/db/schema";

/**
 * Better Auth instance — lazy.
 *
 * Loading is gated on DATABASE_URL + BETTER_AUTH_SECRET because the Drizzle
 * adapter requires a real DB to construct. Until both are set, every server
 * helper (`hooks.server.ts`, the catch-all auth route, etc.) treats auth as
 * "not configured" and lets the placeholder client (src/lib/auth/client.ts)
 * keep driving the dev UI.
 *
 * Wiring expectation:
 *   1. Set DATABASE_URL, BETTER_AUTH_SECRET, BETTER_AUTH_URL in .env
 *   2. Run `pnpm db:push` (or `db:generate` + `db:migrate`) to create tables
 *   3. For billing: set POLAR_ACCESS_TOKEN + POLAR_WEBHOOK_SECRET
 *      + POLAR_PRODUCT_ID_PRO
 *
 * Once env is set, swap the client at src/lib/auth/client.ts to:
 *   import { createAuthClient } from "better-auth/svelte";
 *   import { polarClient } from "@polar-sh/better-auth/client";
 *   export const authClient = createAuthClient({ plugins: [polarClient()] });
 */

function createAuth() {
	return betterAuth({
		secret: env.BETTER_AUTH_SECRET as string,
		baseURL: env.BETTER_AUTH_URL ?? env.PUBLIC_APP_URL,
		database: drizzleAdapter(getDb(), { provider: "pg", schema }),
		emailAndPassword: {
			enabled: true,
			requireEmailVerification: false,
		},
		socialProviders: buildSocialProviders(),
		plugins: buildPlugins(),
	});
}

type AuthInstance = ReturnType<typeof createAuth>;

let cached: AuthInstance | null = null;

export function getAuth(): AuthInstance | null {
	if (cached) return cached;
	if (!env.DATABASE_URL || !env.BETTER_AUTH_SECRET) return null;
	cached = createAuth();
	return cached;
}

function buildSocialProviders() {
	const providers: Record<string, { clientId: string; clientSecret: string }> = {};
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
	const polarClient = tryGetPolarClient();
	const proProductId = polarProductIdFor("pro");
	const webhookSecret = env.POLAR_WEBHOOK_SECRET;

	// All-or-nothing: the Polar plugin needs the client, a product to sell,
	// and a webhook secret to verify incoming events. Skip the plugin entirely
	// if any piece is missing so the auth surface still mounts cleanly.
	if (!polarClient || !proProductId || !webhookSecret) return [];

	return [
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
	];
}

/**
 * Polar webhook payloads carry the subscription record under `.data`. The
 * pieces we actually need (customer id, subscription id, status, period
 * end) sit in well-known places — extract defensively so a Polar payload
 * tweak doesn't break our sync.
 */
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
		// We only sell one paid plan today — anything active maps to "pro".
		plan: "pro",
		status,
		currentPeriodEnd,
		cancelAtPeriodEnd,
	});
}

function extractUserId(payload: unknown): string | null {
	const data = (payload as { data?: Record<string, unknown> })?.data ?? {};
	// @polar-sh/better-auth sets `customerExternalId` matching our user.id.
	const v =
		data.customerExternalId ??
		data.customer_external_id ??
		((data.customer as Record<string, unknown> | undefined)?.externalId as
			| string
			| undefined);
	return typeof v === "string" && v.length > 0 ? v : null;
}
