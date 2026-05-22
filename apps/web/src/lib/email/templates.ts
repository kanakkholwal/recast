import {
	ctaButton,
	fallbackLink,
	heading,
	muted,
	paragraph,
	strong,
	wrap,
} from "./layout";
import { sendEmail } from "./transport";

/**
 * Template registry — the only place HTML email markup lives. Each entry
 * returns `{ subject, text, html }`. The transport layer logs the `text`
 * version to stdout in dev so you can copy/click links without a real
 * inbox, while the `html` version is what Resend delivers.
 *
 * To add a template: define its data type, append it to `templates`, then
 * call `sendTemplatedEmail("its-name", to, data)`.
 */

export type TemplateData = {
	"magic-link": { url: string; firstName?: string | null };
	"reset-password": { url: string; firstName?: string | null };
	"team-invitation": {
		url: string;
		teamName: string;
		inviterName: string;
		inviterEmail: string;
	};
};

export type TemplateName = keyof TemplateData;

type Rendered = { subject: string; text: string; html: string };

const templates: {
	[K in TemplateName]: (data: TemplateData[K]) => Rendered;
} = {
	"magic-link": ({ url, firstName }) => {
		const hello = firstName ? `Hi ${firstName},` : "Hi,";
		return {
			subject: "Your Recast sign-in link",
			text:
				`${hello}\n\n` +
				`Click the link below to sign in to Recast. It expires in 10 minutes:\n\n` +
				`${url}\n\n` +
				`If you didn't ask for this, you can safely ignore the email.`,
			html: wrap({
				subject: "Your Recast sign-in link",
				preheader: "One-tap sign-in link, expires in 10 minutes.",
				body:
					heading("Sign in to Recast") +
					paragraph(`${hello.replace(",", "")} — tap below to sign in. The link expires in <strong>10 minutes</strong>.`) +
					ctaButton("Sign in to Recast", url) +
					muted(
						"If you didn't request this, ignore the email — no account changes were made.",
					) +
					fallbackLink(url),
			}),
		};
	},

	"reset-password": ({ url, firstName }) => {
		const hello = firstName ? `Hi ${firstName},` : "Hi,";
		return {
			subject: "Reset your Recast password",
			text:
				`${hello}\n\n` +
				`We received a request to reset your Recast password. Use the\n` +
				`link below to choose a new one:\n\n${url}\n\n` +
				`If you didn't ask for this, you can ignore the email — your\n` +
				`password stays the same.`,
			html: wrap({
				subject: "Reset your Recast password",
				preheader: "Choose a new password for your Recast account.",
				body:
					heading("Reset your password") +
					paragraph(
						`${hello.replace(",", "")} — someone (hopefully you) asked to reset your Recast password.`,
					) +
					ctaButton("Set a new password", url) +
					muted(
						"If this wasn't you, just ignore the email. Your password won't change.",
					) +
					fallbackLink(url),
			}),
		};
	},

	"team-invitation": ({ url, teamName, inviterName, inviterEmail }) => {
		const subject = `${inviterName} invited you to ${teamName} on Recast`;
		return {
			subject,
			text:
				`${inviterName} (${inviterEmail}) invited you to join the team\n` +
				`"${teamName}" on Recast.\n\n` +
				`Open the link below to accept (you'll sign in with this email):\n\n` +
				`${url}\n\n` +
				`The invite expires in 7 days. If you weren't expecting it, you\n` +
				`can ignore the email.`,
			html: wrap({
				subject,
				preheader: `Join ${teamName} on Recast — invite expires in 7 days.`,
				body:
					heading(`You're invited to ${teamName}`) +
					paragraph(
						`${strong(inviterName)} (${inviterEmail}) added you to the team ` +
							`<strong>${escapeText(teamName)}</strong> on Recast. The invite expires in 7 days.`,
					) +
					ctaButton("Accept invitation", url) +
					muted(
						"Wasn't expecting this? Ignore the email — you won't be added to anything.",
					) +
					fallbackLink(url),
			}),
		};
	},
};

/**
 * Send a templated email. Single entrypoint for every transactional message —
 * keeps subjects/copy in one file and rendering centralized.
 */
export async function sendTemplatedEmail<N extends TemplateName>(args: {
	to: string;
	template: N;
	data: TemplateData[N];
	replyTo?: string;
}): Promise<void> {
	const rendered = templates[args.template](args.data);
	await sendEmail({
		to: args.to,
		subject: rendered.subject,
		text: rendered.text,
		html: rendered.html,
		replyTo: args.replyTo,
	});
}

// Tiny helper used only inside this file — anything richer should call
// the exported `strong()` from ./layout. Kept private so callers don't
// hand-roll HTML strings outside the template registry.
function escapeText(s: string): string {
	return s
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;");
}
