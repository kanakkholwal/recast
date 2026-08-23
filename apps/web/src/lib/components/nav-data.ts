import type { IconComponent } from "@recast/icons";
import {
	BarChart3,
	Blocks,
	BookOpen,
	Download,
	FileText,
	Image as ImageIcon,
	Mail,
	Rocket,
	Scissors,
	Sparkles,
	Tag,
	Wrench,
} from "@recast/icons";
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

/**
 * The nav used to carry four bare links, so everything else in the site was
 * only reachable from the footer. These groups open in a shared panel.
 */
export type MenuItem = {
	label: string;
	href: string;
	description: string;
	icon: IconComponent;
	external?: boolean;
};

export type MenuGroup = {
	label: string;
	/** Where the trigger itself goes if someone clicks rather than hovers. */
	href: string;
	items: MenuItem[];
	/** Optional promoted row along the foot of the panel. */
	footer?: { label: string; href: string; hint: string };
};

export const menuGroups: MenuGroup[] = [
	{
		label: "Product",
		href: "/features",
		items: [
			{
				label: "Features",
				href: "/features",
				description: "Everything Recast does, end to end",
				icon: Sparkles,
			},
			{
				label: "Download",
				href: "/download",
				description: "macOS, Windows and Linux builds",
				icon: Download,
			},
			{
				label: "Extensions",
				href: "/extensions",
				description: "Cursors, backdrops and presets",
				icon: Blocks,
			},
			{
				label: "Pricing",
				href: "/pricing",
				description: "Free to record, paid to share at scale",
				icon: Tag,
			},
		],
		footer: {
			label: "Start recording free",
			href: "/download",
			hint: "No account needed",
		},
	},
	{
		label: "Tools",
		href: "/tools",
		items: [
			{
				label: "Screenshot editor",
				href: "/tools/screenshot-editor",
				description: "Backdrops, frames and a 3D tilt",
				icon: ImageIcon,
			},
			{
				label: "MP4 to GIF",
				href: "/tools/mp4-to-gif",
				description: "Turn a clip into an animated GIF",
				icon: Wrench,
			},
			{
				label: "Trim video",
				href: "/tools/trim-video",
				description: "Cut to the part you want, losslessly",
				icon: Scissors,
			},
			{
				label: "Compress video",
				href: "/tools/compress-video",
				description: "Shrink a file to fit an upload cap",
				icon: Rocket,
			},
		],
		footer: {
			label: "Browse all tools",
			href: "/tools",
			hint: "Free, and nothing is uploaded",
		},
	},
	{
		label: "Resources",
		href: "/blog",
		items: [
			{
				label: "Blog",
				href: "/blog",
				description: "What we are building and why",
				icon: BookOpen,
			},
			{
				label: "Changelog",
				href: "/changelog",
				description: "Every release, in order",
				icon: FileText,
			},
			{
				label: "Architecture",
				href: "/architecture",
				description: "How the app is put together",
				icon: BarChart3,
			},
			{
				label: "GitHub",
				href: GITHUB_URL,
				description: "Source, issues and releases",
				icon: Blocks,
				external: true,
			},
		],
	},
];

/** Flat links that sit beside the groups and need no panel. */
export const navLinks: NavLink[] = [PRICING];

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
			TOOLS,
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
