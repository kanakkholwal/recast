/**
 * Jamendo music provider (https://developer.jamendo.com). Searches the CC
 * catalog and, on add, downloads the track locally (via the `download_music_asset`
 * Rust command) so playback is offline-first. Needs a free `client_id` from
 * devportal.jamendo.com. All Jamendo tracks are Creative Commons — carry the
 * `attribution` + `license` through so the app can credit them.
 */

import { invoke } from "@tauri-apps/api/core";
import type { AudioClipSourceProvider, MusicProvider, MusicSearchResult } from "../music";

const JAMENDO_TRACKS = "https://api.jamendo.com/v3.0/tracks/";

/** Build the /tracks search URL (pure — unit-tested). */
export function buildJamendoSearchUrl(clientId: string, query: string, limit = 24): string {
	const p = new URLSearchParams({
		client_id: clientId,
		format: "json",
		limit: String(limit),
		audioformat: "mp32",
		include: "musicinfo licenses",
		namesearch: query,
	});
	return `${JAMENDO_TRACKS}?${p.toString()}`;
}

interface JamendoRawTrack {
	id?: string | number;
	name?: string;
	artist_name?: string;
	duration?: number;
	audio?: string;
	audiodownload?: string;
	audiodownload_allowed?: boolean;
	license_ccurl?: string;
}

/** Map a Jamendo /tracks JSON response to results (pure — unit-tested). */
export function parseJamendoTracks(json: unknown): MusicSearchResult[] {
	const results = (json as { results?: JamendoRawTrack[] } | null)?.results;
	if (!Array.isArray(results)) return [];
	const out: MusicSearchResult[] = [];
	for (const t of results) {
		if (t.id === undefined || t.id === null) continue;
		// Prefer the explicit download URL when allowed; else the streaming URL.
		const url = (t.audiodownload_allowed && t.audiodownload) || t.audio || "";
		if (!url) continue;
		const title = t.name ?? "Untitled";
		const artist = t.artist_name ?? "Unknown artist";
		out.push({
			trackId: String(t.id),
			title,
			artist,
			durationSec: typeof t.duration === "number" ? t.duration : undefined,
			attribution: `"${title}" by ${artist} (Jamendo)`,
			license: t.license_ccurl,
			downloadUrl: url,
			// Stream URL for audition (falls back to the same url when absent).
			previewUrl: t.audio || url,
		});
	}
	return out;
}

export function createJamendoProvider(clientId: string): MusicProvider {
	return {
		id: "jamendo",
		label: "Jamendo",
		async search(query: string): Promise<MusicSearchResult[]> {
			if (!clientId) throw new Error("Add your Jamendo client ID to search.");
			const res = await fetch(buildJamendoSearchUrl(clientId, query));
			if (!res.ok) throw new Error(`Jamendo search failed (${res.status}).`);
			return parseJamendoTracks(await res.json());
		},
		async resolve(result: MusicSearchResult): Promise<AudioClipSourceProvider> {
			const assetPath = await invoke<string>("download_music_asset", {
				url: result.downloadUrl,
				id: `jamendo-${result.trackId}`,
			});
			return {
				kind: "provider",
				providerId: "jamendo",
				trackId: result.trackId,
				assetPath,
				attribution: result.attribution,
				license: result.license,
			};
		},
	};
}
