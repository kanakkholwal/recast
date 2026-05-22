import { dev } from "$app/environment";
import { haveIBeenPwned } from "better-auth/plugins";

import { sendEmail } from "$lib/auth/email";
import { polarProductIdFor } from "$lib/billing/plans";
import { tryGetPolarClient } from "$lib/billing/polar";
import { downgradeToFree, upsertSubscription } from "$lib/billing/sync";
import { getDb } from "$lib/db";
import * as schema from "$lib/db/schema";
import {
	TEAM_PLAN_MEMBER_CAPS,
	USER_TEAM_OWNERSHIP_CAPS,
	member as memberTable,
	organization as organizationTable,
	user as userTable,
} from "$lib/db/schema";
import { publicEnv } from "$lib/env/public";
import { serverEnv } from "$lib/env/server";
import {
	checkout,
	polar,
	portal,
	webhooks,
} from "@polar-sh/better-auth";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { admin, magicLink, organization } from "better-auth/plugins";
import { and, count, eq } from "drizzle-orm";

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
		// Auto-create a default org the first time a user row appears, so
		// every signed-in account lands in a team. The org's plan starts at
		// "free"; admins can elevate it from /admin/teams/[id].
		databaseHooks: {
			user: {
				create: {
					after: async (createdUser) => {
						await ensureDefaultTeamForUser({
							id: createdUser.id,
							name: createdUser.name ?? "",
							email: createdUser.email,
						});
					},
				},
			},
		},
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

	// Organization plugin — owns the `organization`, `member`, `invitation`
	// tables and `session.activeOrganizationId`. Caps:
	//
	//   • Per-team member count: read from `organization.plan` via our
	//     TEAM_PLAN_MEMBER_CAPS map. Free = 3, Pro = 50, Enterprise = ∞.
	//   • Per-user team-ownership count: 3 if all owned teams are free;
	//     10 once any owned team is pro/enterprise.
	//
	// `allowUserToCreateOrganization` runs before /organization/create — we
	// return false when the cap is hit; the plugin throws a clean 403.
	const orgPlugin = organization({
		creatorRole: "owner",
		invitationExpiresIn: 7 * 24 * 60 * 60, // 7 days
		allowUserToCreateOrganization: async (u) => {
			const db = getDb();
			// Count teams this user OWNS (members with role=owner), join to org
			// to read each team's plan.
			const owned = await db
				.select({ plan: organizationTable.plan })
				.from(memberTable)
				.innerJoin(
					organizationTable,
					eq(memberTable.organizationId, organizationTable.id),
				)
				.where(and(eq(memberTable.userId, u.id), eq(memberTable.role, "owner")));
			const hasPaidTeam = owned.some((o) => o.plan !== "free");
			const cap = hasPaidTeam
				? USER_TEAM_OWNERSHIP_CAPS.paid
				: USER_TEAM_OWNERSHIP_CAPS.free;
			return owned.length < cap;
		},
		membershipLimit: async (_u, org) => {
			const plan = (org as { plan?: string }).plan ?? "free";
			return TEAM_PLAN_MEMBER_CAPS[plan] ?? TEAM_PLAN_MEMBER_CAPS.free!;
		},
		schema: {
			organization: {
				additionalFields: {
					plan: { type: "string", defaultValue: "free", required: false },
				},
			},
		},
		sendInvitationEmail: async ({ email, organization: org, inviter, id }) => {
			const base = serverEnv().BETTER_AUTH_URL ?? publicEnv().PUBLIC_APP_URL;
			const acceptUrl = `${base.replace(/\/$/, "")}/accept-invitation?id=${id}`;
			const inviterName = inviter.user.name || inviter.user.email;
			await sendEmail({
				to: email,
				subject: `${inviterName} invited you to ${org.name} on Recast`,
				text:
					`${inviterName} (${inviter.user.email}) invited you to join the\n` +
					`team "${org.name}" on Recast.\n\n` +
					`Open the link below to accept (you'll need to sign in with this\n` +
					`email address):\n\n${acceptUrl}\n\n` +
					`If you didn't expect this invite, you can ignore the email — it\n` +
					`expires in 7 days.`,
			});
		},
	});

	return [adminPlugin, linkPlugin, orgPlugin, ...polarPlugins, haveIBeenPwned({
		enabled: !dev,
	})];
}

/**
 * Creates a "{name}'s Team" org for a user if they don't have one yet.
 * Idempotent — safe to call twice (the membership check short-circuits).
 *
 * Skipped silently for waitlist (`status === "pending"`) users so we don't
 * spawn orphan teams for emails the user hasn't activated yet.
 */
async function ensureDefaultTeamForUser(u: {
	id: string;
	name: string;
	email: string;
}): Promise<void> {
	const db = getDb();
	try {
		const [row] = await db
			.select({ status: userTable.status })
			.from(userTable)
			.where(eq(userTable.id, u.id))
			.limit(1);
		if (row?.status === "pending") return;

		const [existing] = await db
			.select({ c: count() })
			.from(memberTable)
			.where(eq(memberTable.userId, u.id));
		if ((existing?.c ?? 0) > 0) return;

		const first = (u.name || u.email.split("@")[0] || "Personal").split(/\s+/)[0]!;
		const orgId = crypto.randomUUID();
		// Slug needs to be unique — suffix with a short id so two "Kanak's
		// Team" rows don't collide on the org.slug unique index.
		const slugBase = first
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/(^-|-$)/g, "")
			|| "team";
		const slug = `${slugBase}-${orgId.slice(0, 6)}`;

		await db.insert(organizationTable).values({
			id: orgId,
			name: `${first}'s Team`,
			slug,
			plan: "free",
		});
		await db.insert(memberTable).values({
			id: crypto.randomUUID(),
			organizationId: orgId,
			userId: u.id,
			role: "owner",
		});
	} catch (err) {
		console.error("[auth] ensureDefaultTeamForUser failed", err);
	}
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
