import { serverEnv } from "$lib/env/server";

/**
 * Outbound mail. Wired to Resend in production; logs to stdout otherwise so
 * magic links + reset links are discoverable during local development.
 *
 * Set `RESEND_API_KEY` + `EMAIL_FROM` to send real mail. Swap to your
 * provider of choice (Postmark, Loops, AWS SES) by replacing the body of
 * this helper — every caller goes through it.
 */

export type EmailMessage = {
	to: string;
	subject: string;
	text: string;
	html?: string;
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
			html: msg.html ?? msg.text.replace(/\n/g, "<br>"),
		}),
	});

	if (!res.ok) {
		const body = await res.text().catch(() => "");
		console.error(`[email] Resend ${res.status}: ${body}`);
	}
}
