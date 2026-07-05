/** Shared accessory-badge styling for RecastCard and RecastRow. */

import type { RecastAccessory } from "./types";

const ACCESSORY_VARIANTS = {
	default: "bg-muted/80 text-muted-foreground border-border/40",
	success: "bg-success/10 text-success border-success/20",
	warning: "bg-warning/10 text-warning border-warning/20",
	destructive: "bg-destructive/10 text-destructive border-destructive/20",
	info: "bg-info/10 text-info border-info/20",
} as const;

/** Tailwind classes for an accessory badge, keyed by its variant. */
export function accessoryClass(a: RecastAccessory): string {
	return ACCESSORY_VARIANTS[a.variant ?? "default"];
}
