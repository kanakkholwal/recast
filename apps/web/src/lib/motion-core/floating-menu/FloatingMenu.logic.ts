import type { MenuButton } from "./FloatingMenu.types";

/** Anchor `target`/`rel` for a button: opens a new tab for external hrefs. */
export function externalAttrs(btn: MenuButton) {
	const isHttp =
		btn.href.startsWith("http://") || btn.href.startsWith("https://");
	const open = btn.external ?? isHttp;
	return open ? { target: "_blank", rel: "noopener noreferrer" } : {};
}

/** Responsive container widths for the collapsed/open menu states. */
export function resolveMenuWidths(width: number): {
	isMobile: boolean;
	maxWidthOpen: string;
	maxWidthInitial: string;
} {
	const isMobile = width < 768;
	const isTablet = width >= 768 && width < 1024;

	let maxWidthOpen = "75%";
	let maxWidthInitial = "60%";

	if (isMobile) {
		maxWidthOpen = "100%";
		maxWidthInitial = "95%";
	} else if (isTablet) {
		maxWidthOpen = "85%";
		maxWidthInitial = "70%";
	}

	return { isMobile, maxWidthOpen, maxWidthInitial };
}
