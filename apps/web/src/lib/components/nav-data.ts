import Mail from "@lucide/svelte/icons/mail";
import { GithubBrand, XBrand } from "@recast/ui/brand-icons";
import type { Component } from "svelte";

// Single source of truth for the site chrome (Navbar + Footer). Both surfaces
// repeat the same product links and external URLs; keep the URLs here so a
// changed handle/repo only edits in one place.

type IconComponent = Component<{ class?: string; size?: number | string }>;

export type NavLink = { label: string; href: string; external?: boolean };

export const GITHUB_URL = "https://github.com/kanakkholwal/recast";
const GITHUB_RELEASES_URL = "https://github.com/kanakkholwal/recast/releases";
const TWITTER_URL = "https://x.com/kanakkholwal";
const DISCORD_URL = "https://discord.gg/rBCuqRsb5";
const CONTACT_EMAIL = "mailto:try-recast@gmail.com";

// Product links shared verbatim between Navbar and Footer.
const FEATURES: NavLink = { label: "Features", href: "/features" };
const EXTENSIONS: NavLink = { label: "Extensions", href: "/extensions" };
// Not a product offering — a resource. They exist to catch people searching for
// a quick browser tool, so they sit under Resources, not Product.
const TOOLS: NavLink = { label: "Tools", href: "/tools" };
const PRICING: NavLink = { label: "Pricing", href: "/pricing" };
const CHANGELOG: NavLink = { label: "Changelog", href: "/changelog" };
const BLOG: NavLink = { label: "Blog", href: "/blog" };

// Inline top-nav links, always visible on desktop. Kept short for a minimal
// bar; everything else lives in the footer.
export const navLinks: NavLink[] = [FEATURES, EXTENSIONS, PRICING, CHANGELOG];

export const footerCols: { title: string; links: NavLink[] }[] = [
	{
		title: "Product",
		links: [
			FEATURES,
			EXTENSIONS,
			PRICING,
			{ label: "Download", href: "/download" },
			{ label: "Join Waitlist", href: "/waitlist" },
		],
	},
	{
		title: "Resources",
		links: [
			TOOLS,
			BLOG,
			{ label: "Documentation", href: "#" },
			{ label: "GitHub", href: GITHUB_URL, external: true },
			{ label: "Releases", href: GITHUB_RELEASES_URL, external: true },
			CHANGELOG,
		],
	},
	{
		title: "Company",
		links: [
			{ label: "Contact", href: CONTACT_EMAIL },
			{ label: "X / Twitter", href: TWITTER_URL, external: true },
			{ label: "Discord", href: DISCORD_URL, external: true },
			{ label: "Privacy Policy", href: "/privacy-policy" },
			{ label: "Terms of Service", href: "/terms-of-service" },
		],
	},
];

export const footerSocials: { icon: IconComponent; href: string; label: string }[] = [
	{ icon: GithubBrand, href: GITHUB_URL, label: "GitHub" },
	{ icon: XBrand, href: TWITTER_URL, label: "X / Twitter" },
	{ icon: Mail, href: CONTACT_EMAIL, label: "Email" },
];
