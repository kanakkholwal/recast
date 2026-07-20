import {
	ctaButton,
	fallbackLink,
	heading,
	muted,
	paragraph,
	strong,
	wrap,
} from "./layout";

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
	"verify-email": { url: string; firstName?: string | null };
	"team-invitation": {
		url: string;
		teamName: string;
		inviterName: string;
		inviterEmail: string;
	};
	"waitlist-approved": { url: string; firstName?: string | null };
	"admin-invite": {
		url: string;
		firstName?: string | null;
		/** Shown so the recipient knows a human put them here, not a leak. */
		inviterName: string;
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
					paragraph(`${hello.replace(",", "")}, tap below to sign in. The link expires in <strong>10 minutes</strong>.`) +
					ctaButton("Sign in to Recast", url) +
					muted(
						"If you didn't request this, you can ignore the email. No account changes were made.",
					) +
					fallbackLink(url),
			}),
		};
	},

	"verify-email": ({ url, firstName }) => {
		const hello = firstName ? `Hi ${firstName},` : "Hi,";
		const helloHtml = firstName ? `Hi ${escapeText(firstName)},` : "Hi,";
		return {
			subject: "Verify your Recast email",
			text:
				`${hello}\n\n` +
				`Confirm this email address to finish setting up your Recast\n` +
				`account. The link below is good for the next 24 hours:\n\n${url}\n\n` +
				`Until you verify, dashboard actions stay read-only.`,
			html: wrap({
				subject: "Verify your Recast email",
				preheader: "Confirm your email to unlock your Recast account.",
				body:
					heading("Confirm your email") +
					paragraph(
						`${helloHtml.replace(",", "")}, tap below to confirm <strong>this</strong> is your email. ` +
							`Until you verify, your Recast dashboard stays read-only.`,
					) +
					ctaButton("Verify email", url, "accent") +
					muted(
						"Link valid for 24 hours. Didn't sign up for Recast? You can ignore this email. No account changes were made.",
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
				`If you didn't ask for this, you can ignore the email. Your\n` +
				`password stays the same.`,
			html: wrap({
				subject: "Reset your Recast password",
				preheader: "Choose a new password for your Recast account.",
				body:
					heading("Reset your password") +
					paragraph(
						`${hello.replace(",", "")}, someone (hopefully you) asked to reset your Recast password.`,
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
				preheader: `Join ${teamName} on Recast. Invite expires in 7 days.`,
				body:
					heading(`You're invited to ${teamName}`) +
					paragraph(
						`${strong(inviterName)} (${inviterEmail}) added you to the team ` +
							`<strong>${escapeText(teamName)}</strong> on Recast. The invite expires in 7 days.`,
					) +
					ctaButton("Accept invitation", url) +
					muted(
						"Wasn't expecting this? You can ignore the email. You won't be added to anything.",
					) +
					fallbackLink(url),
			}),
		};
	},

	"waitlist-approved": ({ url, firstName }) => {
		const hello = firstName ? `Hi ${firstName},` : "Hi,";
		const helloHtml = firstName ? `Hi ${escapeText(firstName)},` : "Hi,";
		const subject = "You're in. Welcome to Recast.";
		return {
			subject,
			text:
				`${hello}\n\n` +
				`Your spot on the Recast waitlist just opened up. Set a password\n` +
				`below and your account is ready:\n\n${url}\n\n` +
				`The link works for 7 days. After that, use "Forgot password" on\n` +
				`the sign-in page to get a fresh one.`,
			html: wrap({
				subject,
				preheader: "Your waitlist spot opened up. Set a password to get started.",
				body:
					heading("You're off the waitlist") +
					paragraph(
						`${helloHtml.replace(",", "")}, your spot just opened up. ` +
							`Set a password and your Recast account is ready to use.`,
					) +
					ctaButton("Set your password", url) +
					muted(
						"This link works for 7 days. After that, use \"Forgot password\" on the sign-in page to get a new one.",
					) +
					fallbackLink(url),
			}),
		};
	},

	"admin-invite": ({ url, firstName, inviterName }) => {
		const hello = firstName ? `Hi ${firstName},` : "Hi,";
		const helloHtml = firstName ? `Hi ${escapeText(firstName)},` : "Hi,";
		const subject = "You're invited to try Recast";
		return {
			subject,
			text:
				`${hello}\n\n` +
				`${inviterName} invited you to Recast. Set a password below and\n` +
				`your account is ready:\n\n${url}\n\n` +
				`The link works for 7 days. Recast is a desktop screen recorder,\n` +
				`you can grab the app once you're signed in.`,
			html: wrap({
				subject,
				preheader: `${inviterName} invited you to Recast. Set a password to get started.`,
				body:
					heading("You're invited to try Recast") +
					paragraph(
						`${helloHtml.replace(",", "")}, ${strong(inviterName)} invited you to Recast. ` +
							`Set a password and your account is ready to use.`,
					) +
					ctaButton("Set your password", url) +
					paragraph(
						`Recast is a desktop screen recorder. Once you're signed in you can ` +
							`download the app and record your first take.`,
					) +
					muted(
						"This link works for 7 days. Wasn't expecting this? You can ignore the email, nothing happens until you set a password.",
					) +
					fallbackLink(url),
			}),
		};
	},
};

/**
 * Renders a template without sending it. Exported so the unit suite can assert
 * on real output (asset URLs, link presence, copy rules) instead of trusting
 * that a string built across four helpers came out right.
 */
export function renderTemplate<N extends TemplateName>(
	name: N,
	data: TemplateData[N],
): Rendered {
	return templates[name](data);
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
