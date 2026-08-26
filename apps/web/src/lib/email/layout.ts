/**
 * Inline-styled HTML layout — the only "design system" the email side has.
 * Hex values (not OKLCH) because Gmail/Outlook still cough on modern color
 * functions in 2026. Values are the hex equivalents of the app tokens in
 * packages/design/src/index.css + apps/web/src/app.css, so the email reads
 * as a continuation of the product rather than a separate brand.
 *
 * Color budget follows 60-30-10:
 *   60% — `canvas`, the page behind the card
 *   30% — `cardBg` + `ink` + `border`, the card and its type
 *   10% — `primary`, the brand lime. Ceiling, not a target: on the site the
 *         lime is a rotating word and a pulse dot, and both DESIGN.md files
 *         warn against large lime fills. Here it earns the top stripe of the
 *         card and nothing else, so CTAs stay ink-on-white like the site's
 *         own `variant="dark"` hero button.
 */

export const EMAIL_COLORS = {
	canvas: "#fafafa", // --background (light) oklch(0.985 0 0)
	cardBg: "#ffffff", // --card
	border: "#e0e0e0", // --border (light) oklch(0.91 0 0)
	ink: "#1c1c1c", // --foreground, headings + primary text
	muted: "#71757e", // --muted-foreground
	primary: "#cdec3a", // --primary (dark-mode lime, the recognisable brand hex)
	primaryInk: "#1c1c1c", // text on primary surfaces — never white, lime fails contrast
	buttonBg: "#1c1c1c", // CTA matches the site's foreground-as-button hero CTA
	buttonInk: "#fafafa",
} as const;

/**
 * Absolute, hardcoded on purpose. Deriving this from PUBLIC_APP_URL would ship
 * a localhost `src` to every recipient any time the env var is unset or points
 * at a dev host — a silent, unrecoverable break once the mail is delivered.
 */
export const EMAIL_LOGO_URL = "https://recast.li/email/logo.png";

/**
 * Geist is the product typeface but no mail client will have it, so the stack
 * degrades to the platform UI font rather than a generic serif.
 */
const FONT_STACK =
	"Geist, 'Geist Variable', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Helvetica Neue', Arial, sans-serif";

export type LayoutOptions = {
	subject: string;
	/** Inbox preview text (hidden in body). Keep ≤ 90 chars. */
	preheader?: string;
	/** Markup that goes inside the white card. Already HTML, not text. */
	body: string;
};

/**
 * Wraps content in the shared brand chrome (logo header, card, footer).
 *
 * The mark is a hosted PNG rather than inline SVG or table-drawn pills:
 * Outlook strips inline SVG entirely, and the table-pill workaround drifted
 * from the real logo. `alt` carries the brand name so image-blocking clients
 * still show "Recast" next to the wordmark.
 */
export function wrap({ subject, preheader = "", body }: LayoutOptions): string {
	const year = new Date().getFullYear();
	return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="color-scheme" content="light dark">
<meta name="supported-color-schemes" content="light dark">
<title>${escapeHtml(subject)}</title>
</head>
<body style="margin:0; padding:0; background:${EMAIL_COLORS.canvas}; font-family:${FONT_STACK}; color:${EMAIL_COLORS.ink}; -webkit-text-size-adjust:100%;">
<div style="display:none; max-height:0; overflow:hidden; mso-hide:all; font-size:1px; line-height:1px; color:${EMAIL_COLORS.canvas};">${escapeHtml(preheader)}</div>
<table role="presentation" cellspacing="0" cellpadding="0" border="0" width="100%" style="background:${EMAIL_COLORS.canvas};">
	<tr>
		<td align="center" style="padding:32px 16px;">
			<table role="presentation" cellspacing="0" cellpadding="0" border="0" width="100%" style="max-width:560px;">
				<tr>
					<td style="padding:0 4px 20px;">
						<table role="presentation" cellspacing="0" cellpadding="0" border="0">
							<tr>
								<td style="vertical-align:middle; line-height:0;">
									<img src="${EMAIL_LOGO_URL}" width="36" height="36" alt="Recast" style="display:block; width:36px; height:36px; border:0; outline:none; text-decoration:none;">
								</td>
								<td style="vertical-align:middle; padding-left:10px;">
									<span style="font-size:17px; font-weight:600; color:${EMAIL_COLORS.ink}; letter-spacing:-0.02em;">Recast</span>
								</td>
							</tr>
						</table>
					</td>
				</tr>
				<tr>
					<!-- The card's entire lime budget: a 3px stripe. Matches how the
					     site uses the accent as a signal, never as a surface. -->
					<td style="background:${EMAIL_COLORS.primary}; border-radius:16px 16px 0 0; height:3px; line-height:3px; font-size:1px;">&nbsp;</td>
				</tr>
				<tr>
					<td style="background:${EMAIL_COLORS.cardBg}; border:1px solid ${EMAIL_COLORS.border}; border-top:0; border-radius:0 0 16px 16px; padding:32px 28px;">
						${body}
					</td>
				</tr>
				<tr>
					<td style="padding:20px 8px 0; font-size:12px; line-height:1.6; color:${EMAIL_COLORS.muted};">
						<p style="margin:0;">Recast · the founder-friendly screen recorder.</p>
						<p style="margin:6px 0 0;">Didn't expect this email? It's safe to ignore. We won't email you again.</p>
						<p style="margin:6px 0 0; color:#9a9da3;">© ${year} Recast</p>
					</td>
				</tr>
			</table>
		</td>
	</tr>
</table>
</body>
</html>`;
}

/**
 * Bulletproof CTA button (table-based for Outlook). Pass the full URL —
 * we do no relative-URL resolution here, every caller supplies an absolute.
 *
 * Geometry mirrors the site's hero CTA (`variant="dark" size="xl"`): 16px
 * radius, 14px/28px padding, weight 500. Font size is 16px rather than the
 * site's 18px, which overpowers a 560px-wide card.
 *
 * `tone` defaults to `ink` (dark button, white text) — the same recipe the
 * landing page uses. Pass `accent` for the lime variant, reserved for the
 * one moment where the brand color should be the focal point (verify-email).
 */
export function ctaButton(label: string, url: string, tone: "ink" | "accent" = "ink"): string {
	const bg = tone === "accent" ? EMAIL_COLORS.primary : EMAIL_COLORS.buttonBg;
	const ink = tone === "accent" ? EMAIL_COLORS.primaryInk : EMAIL_COLORS.buttonInk;
	return `<table role="presentation" cellspacing="0" cellpadding="0" border="0" style="margin:20px 0 4px;">
	<tr>
		<td style="background:${bg}; border-radius:16px;">
			<a href="${escapeAttr(url)}" target="_blank" style="display:inline-block; padding:14px 28px; color:${ink}; text-decoration:none; font-size:16px; font-weight:500; letter-spacing:-0.02em;">${escapeHtml(label)}</a>
		</td>
	</tr>
</table>`;
}

export function fallbackLink(url: string): string {
	return `<p style="margin:16px 0 0; font-size:12px; line-height:1.6; color:${EMAIL_COLORS.muted};">
	Or paste this link into your browser:<br>
	<a href="${escapeAttr(url)}" target="_blank" style="color:${EMAIL_COLORS.ink}; word-break:break-all;">${escapeHtml(url)}</a>
</p>`;
}

/**
 * Site heading recipe: semibold, tight tracking, near-1.0 leading.
 * Mirrors the `h1..h6` base layer in app.css.
 */
export function heading(text: string): string {
	return `<h1 style="margin:0 0 12px; font-size:24px; font-weight:600; line-height:1.15; letter-spacing:-0.02em; color:${EMAIL_COLORS.ink};">${escapeHtml(text)}</h1>`;
}

/**
 * Body copy is muted, not full-black — on the site only headings get the full
 * `--foreground`, paragraphs are always `text-muted-foreground`.
 */
export function paragraph(html: string): string {
	return `<p style="margin:0 0 14px; font-size:15px; line-height:1.6; color:${EMAIL_COLORS.muted};">${html}</p>`;
}

export function muted(html: string): string {
	return `<p style="margin:14px 0 0; font-size:12px; line-height:1.6; color:${EMAIL_COLORS.muted};">${html}</p>`;
}

function escapeHtml(s: string): string {
	return s
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;")
		.replace(/'/g, "&#39;");
}

function escapeAttr(s: string): string {
	return escapeHtml(s);
}

/** For inline emphasis inside paragraph()/muted() — escapes the value only. */
export function strong(s: string): string {
	return `<strong style="font-weight:600; color:${EMAIL_COLORS.ink};">${escapeHtml(s)}</strong>`;
}
