/**
 * Inline-styled HTML layout — the only "design system" the email side has.
 * Hex values (not OKLCH) because Gmail/Outlook still cough on modern color
 * functions in 2026. Keep the visual language close to recast.nexonauts.com
 * so the email reads as a continuation of the app, not a separate brand.
 */

export const EMAIL_COLORS = {
	canvas: "#f3f3f0",      // page bg — warm off-white, matches --background
	cardBg: "#ffffff",
	border: "#e6e6e3",
	ink: "#1a1a19",         // headings + primary text
	muted: "#6b6b66",       // secondary text
	primary: "#cdec3a",     // brand lime accent
	primaryInk: "#1a1a19",  // text on primary surfaces
	buttonBg: "#1a1a19",    // CTA bg matches site (foreground-as-button)
	buttonInk: "#ffffff",
} as const;

const FONT_STACK =
	"-apple-system, BlinkMacSystemFont, 'Segoe UI', Geist, 'Helvetica Neue', Arial, sans-serif";

export type LayoutOptions = {
	subject: string;
	/** Inbox preview text (hidden in body). Keep ≤ 90 chars. */
	preheader?: string;
	/** Markup that goes inside the white card. Already HTML, not text. */
	body: string;
};

/**
 * Wraps content in the shared brand chrome (logo header, card, footer).
 * No external assets — the wordmark renders as styled HTML so dark-mode
 * inverters and image-blocking clients can't break it.
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
								<td style="vertical-align:middle;">
									<span style="display:inline-block; width:28px; height:28px; background:${EMAIL_COLORS.ink}; border-radius:8px;"></span>
								</td>
								<td style="vertical-align:middle; padding-left:10px;">
									<span style="font-size:16px; font-weight:600; color:${EMAIL_COLORS.ink}; letter-spacing:-0.01em;">Recast</span>
								</td>
							</tr>
						</table>
					</td>
				</tr>
				<tr>
					<td style="background:${EMAIL_COLORS.cardBg}; border:1px solid ${EMAIL_COLORS.border}; border-radius:16px; padding:32px 28px;">
						${body}
					</td>
				</tr>
				<tr>
					<td style="padding:20px 8px 0; font-size:12px; line-height:1.6; color:${EMAIL_COLORS.muted};">
						<p style="margin:0;">Recast · the founder-friendly screen recorder.</p>
						<p style="margin:6px 0 0;">Didn't expect this email? It's safe to ignore — we won't email you again.</p>
						<p style="margin:6px 0 0; color:#999996;">© ${year} Recast</p>
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
 */
export function ctaButton(label: string, url: string): string {
	return `<table role="presentation" cellspacing="0" cellpadding="0" border="0" style="margin:8px 0;">
	<tr>
		<td style="background:${EMAIL_COLORS.buttonBg}; border-radius:10px;">
			<a href="${escapeAttr(url)}" target="_blank" style="display:inline-block; padding:12px 22px; color:${EMAIL_COLORS.buttonInk}; text-decoration:none; font-size:14px; font-weight:600; letter-spacing:-0.01em;">${escapeHtml(label)}</a>
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

export function heading(text: string): string {
	return `<h1 style="margin:0 0 12px; font-size:20px; font-weight:600; letter-spacing:-0.015em; color:${EMAIL_COLORS.ink};">${escapeHtml(text)}</h1>`;
}

export function paragraph(html: string): string {
	return `<p style="margin:0 0 14px; font-size:15px; line-height:1.55; color:${EMAIL_COLORS.ink};">${html}</p>`;
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
