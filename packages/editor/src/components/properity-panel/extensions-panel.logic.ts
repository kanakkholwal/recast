/**
 * Extensions helpers shared by ExtensionsPanel and ExtensionDetailsDialog: both
 * enumerate a pack's `contributes`. Counting, registry-update checks, and the
 * "Includes" grouping live here so the two views can't drift.
 */

import { hasUpdate, type RegistryIndexEntry } from "../../lib/extensions";
import type {
	ExtensionContributions,
	ExtensionManifest,
	InstalledExtension,
} from "../../lib/wire-types";

/** Contribution kinds a pack can declare, in display order. */
const CONTRIBUTION_KEYS: (keyof ExtensionContributions)[] = [
	"cursors",
	"backgrounds",
	"gradients",
	"colors",
	"easings",
	"smoothings",
	"captionPresets",
];

/** Total number of contributed items across every kind in a pack. */
export function contribCount(ext: InstalledExtension): number {
	const c = ext.manifest.contributes ?? {};
	return CONTRIBUTION_KEYS.reduce((n, k) => n + (c[k]?.length ?? 0), 0);
}

/** Whether the registry index has a newer version of an installed pack. */
export function updateAvailableFor(
	ext: InstalledExtension,
	entryById: Map<string, RegistryIndexEntry>,
): boolean {
	return hasUpdate(ext.manifest.version, entryById.get(ext.manifest.id)?.version);
}

/** How many installed packs have a registry update available. */
export function countUpdates(
	installed: InstalledExtension[],
	entryById: Map<string, RegistryIndexEntry>,
): number {
	return installed.filter((ext) => updateAvailableFor(ext, entryById)).length;
}

export interface ContributionGroup<Icon> {
	key: string;
	label: string;
	icon: Icon;
	items: string[];
}

/**
 * Non-empty { label, icon, items } sections for a manifest's contributions.
 * `defs` supplies display order, labels and (opaque) icons; item labels fall
 * back to their id. Icon is generic so this stays free of any Svelte import.
 */
export function buildContributionGroups<Icon>(
	manifest: ExtensionManifest | null,
	defs: { key: keyof ExtensionContributions; label: string; icon: Icon }[],
): ContributionGroup<Icon>[] {
	const c = manifest?.contributes ?? {};
	return defs
		.map((d) => ({
			key: d.key as string,
			label: d.label,
			icon: d.icon,
			items: ((c[d.key] ?? []) as Array<{ label?: string; id: string }>).map(
				(it) => it.label ?? it.id,
			),
		}))
		.filter((g) => g.items.length > 0);
}
