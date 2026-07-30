import type { IconComponent } from "@recast/icons";
import { BrandApple, BrandLinux, BrandWindows, Share2 } from "@recast/icons";
import type { Platform } from "@tauri-apps/plugin-os";

export interface ShareTarget {
	icon: IconComponent;
	/** What the OS calls its own sheet, so the tile names the thing it opens. */
	label: string;
}

/** Neutral fallback for a platform with no mark we can vouch for. */
const GENERIC: ShareTarget = { icon: Share2, label: "System share" };

/**
 * Icon + label for the host OS's share sheet.
 *
 * Takes the platform rather than calling `platform()` itself: that throws
 * outside a Tauri webview, so a module-level call would make this unimportable
 * from tests and from any browser build.
 */
export function shareTargetFor(platform: Platform | null | undefined): ShareTarget {
	switch (platform) {
		case "windows":
			return { icon: BrandWindows, label: "Windows share" };
		case "macos":
		case "ios":
			return { icon: BrandApple, label: "Share sheet" };
		case "linux":
			return { icon: BrandLinux, label: "System share" };
		default:
			return GENERIC;
	}
}
