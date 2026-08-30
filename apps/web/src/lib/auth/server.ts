import { checkout, polar, portal, webhooks } from "@polar-sh/better-auth";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import {
	admin,
	bearer,
	deviceAuthorization,
	haveIBeenPwned,
	magicLink,
	organization,
} from "better-auth/plugins";
import { and, count, eq } from "drizzle-orm";
import { dev } from "$app/environment";
import { clearCheckoutIntent, resolveCheckoutWorkspace } from "$lib/billing/intent";
import { limitsFor, planOf, polarProductIdFor } from "$lib/billing/plans";
import { tryGetPolarClient } from "$lib/billing/polar";
import {
	downgradeToFree,
	findWorkspaceByPolarSubscription,
	upsertSubscription,
} from "$lib/billing/sync";
import { getDb } from "$lib/db";
import * as schema from "$lib/db/schema";
import {
	member as memberTable,
	organization as organizationTable,
	USER_TEAM_OWNERSHIP_CAPS,
	user as userTable,
} from "$lib/db/schema";
import { sendTemplatedEmail } from "$lib/email";
import { publicEnv } from "$lib/env/public";
import { serverEnv } from "$lib/env/server";

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
		// Proxies terminate the client TCP connection, so without these the session ipAddress is the proxy's; most specific first.
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
		// `status` is app-owned, separate from the plugin-owned `role`; surfaced on session.user for the dashboard load.
		user: {
			additionalFields: {
				status: { type: "string", defaultValue: "active", required: false },
				defaultWorkspaceId: { type: "string", required: false },
			},
		},
		emailAndPassword: {
			enabled: true,
			// Sign-in isn't blocked on verification (a locked-out user could never recover); the dashboard layout redirects instead.
			requireEmailVerification: false,
			sendResetPassword: async ({ user, url }) => {
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
			// Invitees and waitlist activations are minted already verified, so they skip this and land on the dashboard.
			sendOnSignUp: true,
			autoSignInAfterVerification: true,
			expiresIn: 60 * 60 * 24, // 24h
			sendVerificationEmail: async ({ user, url }) => {
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
		// Every signed-in account lands in a team; the org starts on 'free' and admins can elevate it.
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
			// Private-beta 'pending' rows had no team, so the first session promotes them in place instead of locking them out.
			session: {
				create: {
					after: async (createdSession) => {
						await activatePendingUser(createdSession.userId);
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
	// Dev only: accept the localhost ports the Tauri shell and the web dev server hit /api/auth from.
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

/**
 * Which OAuth buttons the auth pages may render. Env-driven rather than
 * hardcoded so an unconfigured provider never shows a button that dead-ends
 * on a Better Auth "provider not found".
 */
export function enabledSocialProviders(): SocialProviderId[] {
	return Object.keys(buildSocialProviders()) as SocialProviderId[];
}

export type SocialProviderId = "github" | "google";

function buildPlugins() {
	// Admin plugin owns role, banned, banReason and banExpires on user plus impersonatedBy on session, with built-in 403 for non-admins.
	const adminPlugin = admin({
		defaultRole: "user",
		adminRoles: ["admin"],
		impersonationSessionDuration: 60 * 60, // 1h
	});

	const linkPlugin = magicLink({
		// Existing users only: /signup owns account creation, so an unknown email can't mint a nameless account.
		disableSignUp: true,
		expiresIn: 60 * 10,
		sendMagicLink: async ({ email, url }) => {
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
								onSubscriptionActive: async (payload) => handleSubscriptionEvent(payload),
								onSubscriptionUpdated: async (payload) => handleSubscriptionEvent(payload),
								onSubscriptionCanceled: async (payload) => handleSubscriptionEnded(payload),
								onSubscriptionRevoked: async (payload) => handleSubscriptionEnded(payload),
							}),
						],
					}),
				]
			: [];

	// Owns the organization, member and invitation tables; `allowUserToCreateOrganization` returns false at the cap and the plugin throws a clean 403.
	const orgPlugin = organization({
		creatorRole: "owner",
		invitationExpiresIn: 7 * 24 * 60 * 60, // 7 days
		allowUserToCreateOrganization: async (u) => {
			const db = getDb();
			// Count teams this user OWNS (role=owner), joined to org to read each team's plan.
			const owned = await db
				.select({ plan: organizationTable.plan })
				.from(memberTable)
				.innerJoin(organizationTable, eq(memberTable.organizationId, organizationTable.id))
				.where(and(eq(memberTable.userId, u.id), eq(memberTable.role, "owner")));
			const hasPaidTeam = owned.some((o) => o.plan !== "free");
			const cap = hasPaidTeam ? USER_TEAM_OWNERSHIP_CAPS.paid : USER_TEAM_OWNERSHIP_CAPS.free;
			return owned.length < cap;
		},
		membershipLimit: async (_u, org) => {
			const o = org as { plan?: string; seatLimit?: number | null };
			// A negotiated seat count overrides the plan's ceiling.
			return limitsFor(planOf(o.plan).id, { seatLimit: o.seatLimit }).members;
		},
		schema: {
			organization: {
				additionalFields: {
					plan: { type: "string", defaultValue: "free", required: false },
					// Needed on the org object `membershipLimit` receives.
					seatLimit: { type: "number", required: false },
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

	// Device grant (RFC 8628): the session is created during the DESKTOP's poll, so its ipAddress and userAgent are the device's and revocable. Keep `validateClient` tight.
	const RECAST_DEVICE_CLIENTS = new Set(["recast-desktop"]);
	const devicePlugin = deviceAuthorization({
		verificationUri: "/device",
		expiresIn: "5h",
		interval: "5s",
		userCodeLength: 8,
		validateClient: async (clientId) => RECAST_DEVICE_CLIENTS.has(clientId),
		// better-auth 1.6.11 declares `schema` as a required `z.custom()`, so `{}` satisfies the parse and falls through to the default.
		schema: {},
	});

	// Bearer plugin: `/device/token` returns `session.token`, which otherwise only works via a cookie the desktop client doesn't carry.
	const bearerPlugin = bearer();

	return [
		adminPlugin,
		linkPlugin,
		orgPlugin,
		devicePlugin,
		bearerPlugin,
		...polarPlugins,
		haveIBeenPwned({
			enabled: !dev,
		}),
	];
}

/**
 * Creates a "{name}'s Team" org for a user if they don't have one yet.
 * Idempotent — safe to call twice (the membership check short-circuits).
 *
 * Skipped silently for waitlist (`status === "pending"`) users so we don't
 * spawn orphan teams for emails nobody has claimed. Activation has to call
 * this again — see [activatePendingUser] and the admin invite actions.
 */
export async function ensureDefaultTeamForUser(u: {
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
		// Suffix a short id so two identically named teams don't collide on the org.slug unique index.
		const slugBase =
			first
				.toLowerCase()
				.replace(/[^a-z0-9]+/g, "-")
				.replace(/(^-|-$)/g, "") || "team";
		const slug = `${slugBase}-${orgId.slice(0, 6)}`;

		// One transaction: a failed member insert would leave an ownerless org that still counts against the cap.
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
 * Promotes a leftover `status: "pending"` waitlist row to a real account the
 * first time it gets a session, and gives it the team `ensureDefaultTeamForUser`
 * refused to create while it was pending. No-op for accounts already active.
 */
async function activatePendingUser(userId: string): Promise<void> {
	const db = getDb();
	try {
		const [row] = await db
			.select({
				status: userTable.status,
				name: userTable.name,
				email: userTable.email,
			})
			.from(userTable)
			.where(eq(userTable.id, userId))
			.limit(1);
		if (row?.status !== "pending") return;

		await db
			.update(userTable)
			.set({ status: "active", updatedAt: new Date() })
			.where(eq(userTable.id, userId));
		await ensureDefaultTeamForUser({
			id: userId,
			name: row.name ?? "",
			email: row.email,
		});
	} catch (err) {
		console.error("[auth] activatePendingUser failed", err);
	}
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
	const cancelAtPeriodEnd = Boolean(data.cancelAtPeriodEnd ?? data.cancel_at_period_end ?? false);

	if (!userId || !polarSubscriptionId) return;

	// Renewals arrive after the intent is consumed, so the existing row wins.
	const known = await findWorkspaceByPolarSubscription(polarSubscriptionId);
	const target = known
		? ({ ok: true, organizationId: known, seats: 3 } as const)
		: await resolveCheckoutWorkspace(userId);
	if (!target.ok) {
		console.error(
			`[billing] dropped subscription ${polarSubscriptionId}: ${target.reason} for user ${userId}`,
		);
		return;
	}

	const quantity = Number(data.quantity ?? data.seats ?? target.seats);

	await upsertSubscription({
		organizationId: target.organizationId,
		userId,
		polarCustomerId,
		polarSubscriptionId,
		plan: "pro",
		seats: Number.isFinite(quantity) && quantity > 0 ? quantity : target.seats,
		status,
		currentPeriodEnd,
		cancelAtPeriodEnd,
	});
	await clearCheckoutIntent(userId);
}

/** Cancel/revoke resolve from the stored subscription — the intent is long gone. */
async function handleSubscriptionEnded(payload: unknown): Promise<void> {
	const data = (payload as { data?: Record<string, unknown> })?.data ?? {};
	const polarSubscriptionId = String(data.id ?? "");
	const organizationId = polarSubscriptionId
		? await findWorkspaceByPolarSubscription(polarSubscriptionId)
		: null;
	if (!organizationId) {
		console.error(`[billing] could not resolve workspace to downgrade for ${polarSubscriptionId}`);
		return;
	}
	await downgradeToFree(organizationId);
}

function extractUserId(payload: unknown): string | null {
	const data = (payload as { data?: Record<string, unknown> })?.data ?? {};
	const v =
		data.customerExternalId ??
		data.customer_external_id ??
		((data.customer as Record<string, unknown> | undefined)?.externalId as string | undefined);
	return typeof v === "string" && v.length > 0 ? v : null;
}
