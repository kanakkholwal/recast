import { error, json } from "@sveltejs/kit";
import { and, eq } from "drizzle-orm";
import { z } from "zod";
import { getDb } from "$lib/db";
import { share, shareMember } from "$lib/db/schema";
import { sendEmail } from "$lib/email";
import {
	ctaButton,
	fallbackLink,
	heading,
	muted,
	paragraph,
	strong,
	wrap,
} from "$lib/email/layout";
import { enforceRateLimit } from "$lib/server/rate-limit";
import { grantToken } from "$lib/share/grant";
import { emailField } from "$lib/validation/email";
import type { RequestHandler } from "./$types";

const BodySchema = z.object({
	email: emailField(),
});

/**
 * POST /api/share/[id]/claim
 *
 * Account-less access request for a `selected` (invite-only) share. The
 * viewer submits an email; if it's on the share's allowlist we email a
 * one-click verify link (see ../claim/verify). Mirrors a magic link, but
 * scoped to this share — it never creates a Recast account, which matters
 * because sign-up is waitlist-gated.
 *
 * Response is intentionally generic ("if you're on the list, check your
 * mail") so the endpoint can't be used to enumerate who was invited.
 */
export const POST: RequestHandler = async ({ params, request, url, getClientAddress }) => {
	// Cap per share and IP: each success emails an allowlisted invitee, so this is the email-bomb lever.
	const limited = await enforceRateLimit(
		{ getClientAddress },
		{ bucket: "share-claim", id: params.id, limit: 5, windowMs: 60_000 },
	);
	if (limited) return limited;

	let raw: unknown;
	try {
		raw = await request.json();
	} catch {
		error(400, "Invalid JSON body");
	}
	const parsed = BodySchema.safeParse(raw);
	if (!parsed.success) {
		error(422, parsed.error.issues[0]?.message ?? "Enter a valid email address");
	}
	const { email } = parsed.data;

	const db = getDb();
	const [s] = await db
		.select({ slug: share.slug, visibility: share.visibility })
		.from(share)
		.where(eq(share.slug, params.id))
		.limit(1);
	if (!s) error(404, "Share not found");
	if (s.visibility !== "selected") {
		error(400, "This share isn't invite-only");
	}

	// Not on the allowlist falls through to the same generic reply, so invited emails can't be told from others.
	const [allowed] = await db
		.select({ id: shareMember.id })
		.from(shareMember)
		.where(and(eq(shareMember.shareSlug, s.slug), eq(shareMember.email, email)))
		.limit(1);

	if (allowed) {
		const token = await grantToken(s.slug, email);
		const verifyUrl = `${url.origin}/api/share/${encodeURIComponent(
			s.slug,
		)}/claim/verify?e=${encodeURIComponent(email)}&t=${token}`;
		const shareUrl = `${url.origin}/share/${encodeURIComponent(s.slug)}`;

		await sendEmail({
			to: email,
			subject: "Your access link for a Recast recording",
			text: `You were invited to view a private Recast recording.\n\nOpen this link to get access:\n${verifyUrl}\n\nThe recording lives at ${shareUrl}. If you didn't expect this, you can ignore this email.`,
			html: wrap({
				subject: "Your Recast access link",
				preheader: "Open the recording you were invited to.",
				body: [
					heading("You're on the list"),
					paragraph(
						`You were invited to view a private Recast recording. Click below to unlock it — no account needed.`,
					),
					ctaButton("View the recording", verifyUrl, "accent"),
					fallbackLink(verifyUrl),
					muted(
						`This link is tied to ${strong(email)}. If you didn't request access, you can ignore this email.`,
					),
				].join("\n"),
			}),
		});
	}

	return json({ ok: true });
};
