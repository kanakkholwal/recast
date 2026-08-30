import { describe, expect, it } from "vitest";
import { EMAIL_COLORS, EMAIL_LOGO_URL } from "./layout";
import { renderTemplate, type TemplateName } from "./templates.logic";

const ALL: { name: TemplateName; data: Record<string, unknown> }[] = [
	{ name: "magic-link", data: { url: "https://recast.li/m/1", firstName: "Kanak" } },
	{ name: "reset-password", data: { url: "https://recast.li/r/1", firstName: null } },
	{ name: "verify-email", data: { url: "https://recast.li/v/1", firstName: "Kanak" } },
	{
		name: "team-invitation",
		data: {
			url: "https://recast.li/accept-invitation?id=1",
			teamName: "Acme",
			inviterName: "Kanak",
			inviterEmail: "k@e.com",
		},
	},
	{
		name: "waitlist-approved",
		data: { url: "https://recast.li/reset-password?token=t", firstName: "Kanak" },
	},
	{
		name: "admin-invite",
		data: {
			url: "https://recast.li/reset-password?token=t",
			firstName: "Kanak",
			inviterName: "Kanak",
		},
	},
];

describe.each(ALL)("$name", ({ name, data }) => {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const r = renderTemplate(name, data as any);

	it("has a subject and both bodies", () => {
		expect(r.subject.length).toBeGreaterThan(0);
		expect(r.text.length).toBeGreaterThan(0);
		expect(r.html).toContain("<!DOCTYPE html>");
	});

	it("puts the action URL in both the html and the plain-text body", () => {
		// Dev without a Resend key prints only `text`, so the link must survive there or local testing is impossible.
		expect(r.text).toContain(data.url);
		expect(r.html).toContain(data.url as string);
	});

	it("references the hosted logo, not an inline asset", () => {
		expect(r.html).toContain(EMAIL_LOGO_URL);
		expect(r.html).toContain('alt="Recast"');
		// Inline SVG gets stripped by Outlook; if one creeps back in, catch it.
		expect(r.html).not.toContain("<svg");
	});

	it("uses hex colors only, since Gmail and Outlook choke on oklch", () => {
		expect(r.html).not.toMatch(/oklch|color-mix|var\(--/);
	});

	it("keeps em dashes out of the copy", () => {
		// House style rule, and they read as AI-generated in customer-facing mail.
		expect(r.subject).not.toContain("—");
		expect(r.text).not.toContain("—");
	});
});

describe("invite and approval copy", () => {
	it("tells waitlist users their spot opened and how long the link lasts", () => {
		const r = renderTemplate("waitlist-approved", {
			url: "https://recast.li/reset-password?token=t",
			firstName: "Kanak",
		});
		expect(r.subject).toBe("You're in. Welcome to Recast.");
		expect(r.html).toContain("Set your password");
		expect(r.text).toContain("7 days");
	});

	it("names the inviter so the invite doesn't read as a leak", () => {
		const r = renderTemplate("admin-invite", {
			url: "https://recast.li/reset-password?token=t",
			firstName: "Kanak",
			inviterName: "Ada Lovelace",
		});
		expect(r.subject).toBe("You're invited to try Recast");
		expect(r.html).toContain("Ada Lovelace");
		expect(r.text).toContain("Ada Lovelace");
	});

	it("greets without a dangling comma when no name is known", () => {
		const r = renderTemplate("waitlist-approved", {
			url: "https://recast.li/x",
			firstName: null,
		});
		expect(r.text.startsWith("Hi,")).toBe(true);
		expect(r.text).not.toContain("Hi ,");
	});

	it("escapes an inviter name so it can't inject markup", () => {
		const r = renderTemplate("admin-invite", {
			url: "https://recast.li/x",
			firstName: null,
			inviterName: "<script>alert(1)</script>",
		});
		expect(r.html).not.toContain("<script>alert(1)</script>");
		expect(r.html).toContain("&lt;script&gt;");
	});
});

describe("EMAIL_COLORS", () => {
	it("keeps CTA text off the lime, which fails contrast", () => {
		// #cdec3a is ~92% lightness; white text on it is unreadable.
		expect(EMAIL_COLORS.primaryInk).toBe(EMAIL_COLORS.ink);
	});

	it("is all hex", () => {
		for (const v of Object.values(EMAIL_COLORS)) {
			expect(v).toMatch(/^#[0-9a-f]{6}$/i);
		}
	});
});
