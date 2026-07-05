import type { ClassValue } from "clsx";
import type { Component, Snippet } from "svelte";

// Matches the prop surface Lucide Svelte 5 components expose. Kept loose
// so consumers can also pass a tabler/heroicons-style component.
export type IconComponent = Component<{ class?: string; size?: number | string }>;

export type MenuVariant = "default" | "muted";

export interface MenuLink {
	/**
	 * The text to display for the link.
	 */
	label: string;
	/**
	 * The URL the link points to.
	 */
	href: string;
}

export interface MenuButton {
	/**
	 * The text to display on the button. When `iconOnly` is true the label
	 * is hidden visually but still used as the default `aria-label`.
	 */
	label: string;
	/**
	 * The URL the button links to.
	 */
	href: string;
	/**
	 * Optional icon rendered before the label. Pass a Lucide / icon
	 * component, not an instance.
	 */
	icon?: IconComponent;
	/**
	 * If true, hide the visible label and render the icon only. Falls
	 * back to a `aria-label` derived from `ariaLabel ?? label`.
	 */
	iconOnly?: boolean;
	/**
	 * Override for `aria-label`. Useful when `label` is too short for
	 * assistive tech ("GitHub" vs "Recast on GitHub").
	 */
	ariaLabel?: string;
	/**
	 * If true, open the link in a new tab with safe `rel` defaults.
	 * Defaults to true when `href` starts with http(s):// — set to false
	 * to force same-tab navigation for an external link.
	 */
	external?: boolean;
}

export interface MenuGroup {
	/**
	 * The title of the menu group, displayed above the links.
	 */
	title: string;
	/**
	 * The visual style variant of the group.
	 * 'muted' adds a background color.
	 */
	variant?: MenuVariant;
	/**
	 * Array of links to display within this group.
	 */
	links: MenuLink[];
}

export interface FloatingMenuClasses {
	root?: ClassValue;
	overlay?: ClassValue;
	header?: ClassValue;
	toggleButton?: ClassValue;
	toggleLine?: ClassValue;
	logo?: ClassValue;
	actions?: ClassValue;
	primaryButton?: ClassValue;
	secondaryButton?: ClassValue;
	tertiaryButton?: ClassValue;
	menuWrapper?: ClassValue;
	grid?: ClassValue;
	group?: ClassValue;
	groupMuted?: ClassValue;
	groupTitle?: ClassValue;
	link?: ClassValue;
	linkText?: ClassValue;
	linkUnderline?: ClassValue;
	divider?: ClassValue;
}

export interface Props {
	/**
	 * Groups of links to display in the menu.
	 */
	menuGroups: MenuGroup[];
	/**
	 * Snippet for the logo icon (and optional text).
	 */
	logo?: Snippet;
	/**
	 * Configuration for the primary button in the header.
	 */
	primaryButton?: MenuButton;
	/**
	 * Configuration for the secondary button in the header.
	 */
	secondaryButton?: MenuButton;
	/**
	 * Optional tertiary button — useful for an icon-only action like
	 * GitHub. Rendered to the LEFT of secondaryButton; hidden on mobile
	 * via the same `md:flex` breakpoint as secondary.
	 */
	tertiaryButton?: MenuButton;
	/**
	 * Additional classes for the container.
	 */
	class?: string;
	/**
	 * Additional classes for specific menu slots.
	 */
	classes?: FloatingMenuClasses;
	/**
	 * The target element or selector to append the menu to.
	 * Useful for containment in demos or specific containers.
	 * @default "body"
	 */
	portalTarget?: HTMLElement | string;
}
