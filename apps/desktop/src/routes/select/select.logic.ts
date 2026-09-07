/** Source-list mappers + filters for the source-selector window. */

import type { DisplayInfo, LastSource, WindowInfo } from "$lib/recorder-types";

export type TargetSource = {
	type: "monitor" | "window" | "region";
	id: number;
	label: string;
	appName?: string;
	thumbnail: string | null;
	resolution?: string;
	/** Monitor refresh rate in Hz (monitors only); caps the useful capture fps. */
	refreshHz?: number;
	region?: {
		x: number;
		y: number;
		width: number;
		height: number;
	};
};

/** Displays + non-empty-titled windows → a flat, tabbable source list. */
export function buildSources(displays: DisplayInfo[], windows: WindowInfo[]): TargetSource[] {
	const next: TargetSource[] = [];
	for (const [i, d] of displays.entries()) {
		next.push({
			type: "monitor",
			id: d.id,
			label: d.isPrimary ? "Primary Display" : `Display ${i + 1}`,
			thumbnail: d.thumbnail,
			resolution: `${d.width} × ${d.height}`,
			refreshHz: d.refreshHz || undefined,
		});
	}
	windows.forEach((w) => {
		if (w.title?.trim()) {
			next.push({
				type: "window",
				id: w.id,
				label: w.title,
				appName: w.appName,
				thumbnail: w.thumbnail,
				resolution: `${w.width} × ${w.height}`,
			});
		}
	});
	return next;
}

/** A `region-selected` overlay event → a region source. */
export function regionEventToSource(ev: {
	x: number;
	y: number;
	width: number;
	height: number;
	label: string;
}): TargetSource {
	return {
		type: "region",
		id: 0,
		label: ev.label,
		thumbnail: null,
		resolution: `${ev.width} × ${ev.height}`,
		region: { x: ev.x, y: ev.y, width: ev.width, height: ev.height },
	};
}

/** Persisted `LastSource` → the "remembered" region tile, or null if not a
 *  fully-specified region. */
export function lastRegionToSource(last: LastSource | null): TargetSource | null {
	if (last?.kind !== "region" || !last.regionWidth || !last.regionHeight) {
		return null;
	}
	return {
		type: "region",
		id: 0,
		label: last.label,
		thumbnail: null,
		resolution: `${last.regionWidth} × ${last.regionHeight}`,
		region: {
			x: last.regionX ?? 0,
			y: last.regionY ?? 0,
			width: last.regionWidth,
			height: last.regionHeight,
		},
	};
}

export function filterByType(sources: TargetSource[], type: TargetSource["type"]): TargetSource[] {
	return sources.filter((s) => s.type === type);
}

/** Identity match on (type, id): a source is "selected" when both agree. */
export function isSameSource(selected: TargetSource | null, source: TargetSource): boolean {
	return selected?.id === source.id && selected?.type === source.type;
}
