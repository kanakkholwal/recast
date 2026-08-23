import type { IconComponent } from "@recast/icons";
import { Mail } from "@recast/icons";
import { GithubBrand, XBrand } from "@recast/ui/brand-icons";

export type IconComponentLocal = import("@recast/icons").IconComponent;

export type NavLink = { label: string; href: string; external?: boolean };

export const GITHUB_URL = "https://github.com/kanakkholwal/recast";
const GITHUB_RELEASES_URL = "https://github.com/kanakkholwal/recast/releases";
const TWITTER_URL = "https://x.com/kanakkholwal";
const DISCORD_URL = "https://discord.gg/rBCuqRsb5";
const CONTACT_EMAIL = "mailto:try-recast@gmail.com";

// Product links shared verbatim between Navbar and Footer.
const FEATURES: NavLink = { label: "Features", href: "/features" };
const EXTENSIONS: NavLink = { label: "Extensions", href: "/extensions" };
const TOOLS: NavLink = { label: "Tools", href: "/tools" };

// Not linked from the chrome yet — the route is still unfinished.
const PLAYGROUND: NavLink = { label: "Playground", href: "/playground" };
void PLAYGROUND;
const PRICING: NavLink = { label: "Pricing", href: "/pricing" };
const CHANGELOG: NavLink = { label: "Changelog", href: "/changelog" };
const BLOG: NavLink = { label: "Blog", href: "/blog" };

const ARCHITECTURE: NavLink = { label: "Architecture", href: "/architecture" };

export const navLinks: NavLink[] = [FEATURES, PRICING, CHANGELOG, TOOLS];

export const footerCols: { title: string; links: NavLink[] }[] = [
	{
		title: "Product",
		links: [
			FEATURES,
			EXTENSIONS,
			PRICING,
			{ label: "Download", href: "/download" },
			{ label: "Sign in", href: "/login" },
		],
	},
	{
		title: "Resources",
		links: [
			// TOOLS,
			BLOG,
			ARCHITECTURE,
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
