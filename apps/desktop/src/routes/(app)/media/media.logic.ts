import { getExtension, isImageFile } from "@recast/editor/lib/format/files";
import type { RecordingEntry } from "$lib/ipc";
import type { LibrarySort } from "$lib/library/list";

export type MediaKind = "video" | "audio" | "image" | "other";
export type MediaTab = "all" | MediaKind;
export type MediaSource = "recording" | "export";

export interface MediaItem {
	entry: RecordingEntry;
	source: MediaSource;
	kind: MediaKind;
}

const VIDEO = new Set(["mp4", "mov", "webm", "mkv", "avi", "m4v", "rec", "recast"]);
const AUDIO = new Set(["mp3", "wav", "m4a", "aac", "ogg", "flac", "opus"]);

export function classify(filename: string): MediaKind {
	if (isImageFile(filename)) return "image";
	const ext = getExtension(filename).toLowerCase();
	if (VIDEO.has(ext)) return "video";
	if (AUDIO.has(ext)) return "audio";
	return "other";
}

export const MEDIA_TABS: { value: MediaTab; label: string }[] = [
	{ value: "all", label: "All" },
	{ value: "video", label: "Videos" },
	{ value: "audio", label: "Audio" },
	{ value: "image", label: "Images" },
	{ value: "other", label: "Other" },
];

export function toItems(recordings: RecordingEntry[], exports: RecordingEntry[]): MediaItem[] {
	return [
		...recordings.map((entry) => ({
			entry,
			source: "recording" as const,
			kind: classify(entry.filename),
		})),
		...exports.map((entry) => ({
			entry,
			source: "export" as const,
			kind: classify(entry.filename),
		})),
	];
}

export function filterMedia(items: MediaItem[], tab: MediaTab, query: string): MediaItem[] {
	const q = query.trim().toLowerCase();
	return items.filter(
		(m) => (tab === "all" || m.kind === tab) && (!q || m.entry.filename.toLowerCase().includes(q)),
	);
}

export function sortMedia(items: MediaItem[], sort: LibrarySort): MediaItem[] {
	const list = items.slice();
	if (sort === "recent") list.sort((a, b) => b.entry.created - a.entry.created);
	else if (sort === "name") list.sort((a, b) => a.entry.filename.localeCompare(b.entry.filename));
	else if (sort === "size") list.sort((a, b) => b.entry.sizeBytes - a.entry.sizeBytes);
	return list;
}

export function countByKind(items: MediaItem[]): Record<MediaTab, number> {
	const counts: Record<MediaTab, number> = {
		all: items.length,
		video: 0,
		audio: 0,
		image: 0,
		other: 0,
	};
	for (const m of items) counts[m.kind]++;
	return counts;
}
