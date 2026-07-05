import { dev } from "$app/environment";
import { bearer, deviceAuthorization, haveIBeenPwned } from "better-auth/plugins";

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
import { sendTemplatedEmail } from "$lib/email";
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
		trustedOrigins: buildTrustedOrigins(),
		database: drizzleAdapter(getDb(), { provider: "pg", schema }),
		// Production hosts the API behind Vercel / Cloudflare / proxies that
		// terminate the client TCP connection — without these headers the
		// session row's ipAddress would be the proxy's, not the user's. Order
		// matters: Better Auth picks the first header that has a value and
		// falls back to the request socket IP, so list the most specific
		// (CDN-injected, harder to spoof when origin is locked down) first.
		// Safe to keep enabled in dev — the headers just aren't present, so
		// it falls through to the socket IP naturally.
		advanced: {
			ipAddress: {
				ipAddressHeaders: [
					"cf-connecting-ip",
					"x-vercel-forwarded-for",
					"x-real-ip",
					"x-forwarded-for",
					"x-client-ip",
				],
				disableIpTracking: false,
			},
		},
		// `status` is an app-owned column, separate from the plugin-owned
		// `role`. Surfaces on session.user so the dashboard load can read it.
		user: {
			additionalFields: {
				status: { type: "string", defaultValue: "active", required: false },
				defaultWorkspaceId: { type: "string", required: false },
			},
		},
		emailAndPassword: {
			enabled: true,
			// In production, public sign-up is closed — the waitlist endpoint
			// creates user rows directly. Dev keeps signup open for iteration.
			disableSignUp: !dev,
			// We don't block sign-IN on verification (locked-out users with a
			// flipped password can't recover). Instead the dashboard layout
			// redirects unverified users to /verify-email — view-only is fine,
			// mutations aren't reachable. See [dashboard/+layout.server.ts].
			requireEmailVerification: false,
			sendResetPassword: async ({ user, url }) => {
				if (await isOnWaitlist(user.email)) return;
				await sendTemplatedEmail({
					to: user.email,
					template: "reset-password",
					data: {
						url,
						firstName: user.name?.split(/\s+/)[0] ?? null,
					},
				});
			},
		},
		emailVerification: {
			// Fire on signup automatically. Invitees + waitlist activations are
			// minted with `emailVerified: true` already, so they skip this and
			// land on the dashboard directly.
			sendOnSignUp: true,
			autoSignInAfterVerification: true,
			expiresIn: 60 * 60 * 24, // 24h
			sendVerificationEmail: async ({ user, url }) => {
				if (await isOnWaitlist(user.email)) return;
				await sendTemplatedEmail({
					to: user.email,
					template: "verify-email",
					data: {
						url,
						firstName: user.name?.split(/\s+/)[0] ?? null,
					},
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

// Production hosts the web app is served from. 
const PRODUCTION_TRUSTED_ORIGINS = [
	"https://recast.li",
	"https://www.recast.li",
	"https://recast.nexonauts.com",
	"https://recast-web.vercel.app",
];

function buildTrustedOrigins(): string[] {
	const env = serverEnv();
	const merged = new Set<string>(PRODUCTION_TRUSTED_ORIGINS);
	// In dev, accept the common localhost ports we serve from so the desktop
	// shell (Tauri uses tauri://localhost / http://localhost) and the web
	// dev server can both hit /api/auth without tripping the origin check.
	if (dev) {
		merged.add("http://localhost:5173");
		merged.add("http://localhost:4420");
		merged.add("http://localhost:4421");
		merged.add("tauri://localhost");
		merged.add("http://tauri.localhost");
	}
	for (const o of env.TRUSTED_ORIGINS) merged.add(o);
	return [...merged];
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
			// Look up the user's name so the template can address them.
			const db = getDb();
			const [row] = await db
				.select({ name: userTable.name })
				.from(userTable)
				.where(eq(userTable.email, email))
				.limit(1);
			await sendTemplatedEmail({
				to: email,
				template: "magic-link",
				data: {
					url,
					firstName: row?.name?.split(/\s+/)[0] ?? null,
				},
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
			await sendTemplatedEmail({
				to: email,
				template: "team-invitation",
				data: {
					url: acceptUrl,
					teamName: org.name,
					inviterName: inviter.user.name || inviter.user.email,
					inviterEmail: inviter.user.email,
				},
			});
		},
	});

	// OAuth 2.0 Device Authorization Grant (RFC 8628) — powers the desktop
	// app's "Sign in to Cloud" flow. The desktop calls /device/code, opens
	// the user's browser to verification_uri_complete (a /device page with
	// the code pre-filled), then polls /device/token until the user approves.
	// On approval the plugin's /device/token handler calls
	// internalAdapter.createSession(user.id) during the desktop's polling
	// request — meaning session.ipAddress and session.userAgent are the
	// DESKTOP's, not the browser's. That's the whole reason this flow exists
	// for us: we get a proper per-device session row we can revoke later.
	//
	// `validateClient` is the only thing standing between us and any random
	// caller driving the device flow; keep the allowlist tight.
	const RECAST_DEVICE_CLIENTS = new Set(["recast-desktop"]);
	const devicePlugin = deviceAuthorization({
		verificationUri: "/device",
		expiresIn: "5h",
		interval: "5s",
		userCodeLength: 8,
		validateClient: async (clientId) => RECAST_DEVICE_CLIENTS.has(clientId),
		// Plugin bug in better-auth 1.6.11: `schema` is declared as a required
		// `z.custom()` (no `.optional()`), so the Zod parse throws if it's
		// missing — even though the field is meant for overriding model/field
		// names (which we don't need). Passing `{}` satisfies the validator
		// and falls through to the plugin's default schema.
		schema: {},
	});

	// Bearer plugin — required for the desktop app to use its session token
	// as `Authorization: Bearer <token>` against /api/auth/get-session,
	// /api/auth/sign-out, and (later) cloud sync endpoints. The device-auth
	// plugin's `/device/token` returns `session.token` as `access_token`;
	// without the bearer plugin that token only works via the session cookie,
	// which the desktop's reqwest client doesn't carry. Order doesn't matter
	// for bearer — it adds request middleware that runs before route handlers.
	const bearerPlugin = bearer();

	return [adminPlugin, linkPlugin, orgPlugin, devicePlugin, bearerPlugin, ...polarPlugins, haveIBeenPwned({
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

		// Both writes in one transaction — a failure on the member insert
		// (e.g. FK violation, connection drop) would otherwise leave an
		// ownerless org behind, which the org-count cap would still count
		// against the user. Either both commit or neither.
		await db.transaction(async (tx) => {
			await tx.insert(organizationTable).values({
				id: orgId,
				name: `${first}'s Team`,
				slug,
				plan: "free",
			});
			await tx.insert(memberTable).values({
				id: crypto.randomUUID(),
				organizationId: orgId,
				userId: u.id,
				role: "owner",
			});
			await tx
				.update(userTable)
				.set({ defaultWorkspaceId: orgId, updatedAt: new Date() })
				.where(eq(userTable.id, u.id));
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
