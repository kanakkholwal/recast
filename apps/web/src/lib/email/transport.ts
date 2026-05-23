import { serverEnv } from "$lib/env/server";

/**
 * Low-level email transport. Resend in production, stdout in dev so links
 * are visible without an inbox round-trip. Every higher-level send goes
 * through `sendTemplatedEmail` (see ./templates) — call this directly only
 * if you need to bypass the templating layer.
 */

export type EmailMessage = {
	to: string;
	subject: string;
	text: string;
	html?: string;
	/** Optional reply-to address; defaults to EMAIL_FROM. */
	replyTo?: string;
};

export async function sendEmail(msg: EmailMessage): Promise<void> {
	const { RESEND_API_KEY: apiKey, EMAIL_FROM: from } = serverEnv();

	if (!apiKey) {
		console.log("\n[email — no provider configured]");
		console.log(`  to:      ${msg.to}`);
		console.log(`  subject: ${msg.subject}`);
		console.log(`  body:    ${msg.text.split("\n").join("\n           ")}`);
		console.log("");
		return;
	}

	const res = await fetch("https://api.resend.com/emails", {
		method: "POST",
		headers: {
			Authorization: `Bearer ${apiKey}`,
			"Content-Type": "application/json",
		},
		body: JSON.stringify({
			from,
			to: [msg.to],
			subject: msg.subject,
			text: msg.text,
			// Resend accepts text-only, but HTML lifts deliverability AND
			// rendering quality — fall back to a naive conversion if a caller
			// somehow bypassed `sendTemplatedEmail` without supplying HTML.
			html: msg.html ?? msg.text.replace(/\n/g, "<br>"),
			reply_to: msg.replyTo,
		}),
	});

	if (!res.ok) {
		const body = await res.text().catch(() => "");
		console.error(`[email] Resend ${res.status}: ${body}`);
	}
}
