import { browser } from "$app/environment";

/**
 * Dashboard data layer.
 *
 * No backend exists yet — this models how the web dashboard will behave once
 * the desktop app gains a backend + OAuth. State is fully reactive and
 * persisted to localStorage so the UI is genuinely functional on dummy data.
 */

export type RecordingSource = "cloud" | "local";

export type Recording = {
	id: string;
	title: string;
	durationSec: number;
	createdAt: number;
	sizeBytes: number;
	source: RecordingSource;
	provider: string | null;
	views: number;
	/** Playable URL. May be a `blob:` URL for session uploads. */
	videoUrl: string;
	/** Poster image; empty string renders a gradient placeholder. */
	posterUrl: string;
};

/** Workspace storage quota used by the sidebar meter. */
export const STORAGE_QUOTA_BYTES = 5 * 1024 ** 3;

const REC_KEY = "recast.dashboard.recordings.v1";
const SET_KEY = "recast.dashboard.settings.v1";

// Stable, public sample media so playback genuinely works on dummy data.
function sample(name: string) {
	return {
		videoUrl: `https://storage.googleapis.com/gtv-videos-bucket/sample/${name}.mp4`,
		posterUrl: `https://storage.googleapis.com/gtv-videos-bucket/sample/images/${name}.jpg`,
	};
}

const DAY = 86_400_000;

function seedRecordings(): Recording[] {
	const now = Date.now();
	return [
		{ id: "rec_walkthrough", title: "Series A — product walkthrough", durationSec: 252, createdAt: now - 1 * DAY, sizeBytes: 191_000_000, source: "cloud", provider: "Cloudinary", views: 48, ...sample("BigBuckBunny") },
		{ id: "rec_onboarding", title: "Onboarding flow v3", durationSec: 158, createdAt: now - 3 * DAY, sizeBytes: 101_000_000, source: "cloud", provider: "Cloudinary", views: 213, ...sample("ElephantsDream") },
		{ id: "rec_changelog", title: "Changelog — sprint 22", durationSec: 64, createdAt: now - 4 * DAY, sizeBytes: 43_000_000, source: "local", provider: null, views: 0, ...sample("ForBiggerBlazes") },
		{ id: "rec_bug", title: "Bug repro — export hang", durationSec: 52, createdAt: now - 6 * DAY, sizeBytes: 33_000_000, source: "local", provider: null, views: 0, ...sample("ForBiggerEscapes") },
		{ id: "rec_teaser", title: "Launch teaser cut", durationSec: 31, createdAt: now - 9 * DAY, sizeBytes: 22_000_000, source: "cloud", provider: "Cloudinary", views: 1024, ...sample("ForBiggerFun") },
		{ id: "rec_support", title: "Support reply — billing", durationSec: 107, createdAt: now - 13 * DAY, sizeBytes: 68_000_000, source: "local", provider: null, views: 0, ...sample("ForBiggerJoyrides") },
	];
}

function readJSON<T>(key: string, fallback: T): T {
	if (!browser) return fallback;
	try {
		const raw = localStorage.getItem(key);
		return raw ? (JSON.parse(raw) as T) : fallback;
	} catch {
		return fallback;
	}
}

/** Blob URLs don't survive a reload — fall back to sample media so the
 *  recording stays playable rather than becoming a dead entry. */
function reconcile(r: Recording): Recording {
	if (r.videoUrl?.startsWith("blob:")) {
		return { ...r, ...sample("WeAreGoingOnBubbles"), posterUrl: "" };
	}
	return r;
}

class RecordingsStore {
	items = $state<Recording[]>([]);

	constructor() {
		const stored = readJSON<Recording[] | null>(REC_KEY, null);
		this.items = (stored ?? seedRecordings()).map(reconcile);
	}

	private persist() {
		if (browser) localStorage.setItem(REC_KEY, JSON.stringify(this.items));
	}

	get usedBytes(): number {
		return this.items.reduce((sum, r) => sum + r.sizeBytes, 0);
	}

	get cloudCount(): number {
		return this.items.filter((r) => r.source === "cloud").length;
	}

	add(rec: Recording) {
		this.items = [rec, ...this.items];
		this.persist();
	}

	remove(id: string) {
		this.items = this.items.filter((r) => r.id !== id);
		this.persist();
	}

	rename(id: string, title: string) {
		this.items = this.items.map((r) =>
			r.id === id ? { ...r, title } : r,
		);
		this.persist();
	}

	setSource(id: string, source: RecordingSource) {
		this.items = this.items.map((r) =>
			r.id === id
				? { ...r, source, provider: source === "cloud" ? "Cloudinary" : null }
				: r,
		);
		this.persist();
	}

	reset() {
		this.items = seedRecordings();
		this.persist();
	}
}

export type DashboardSettings = {
	profile: { name: string; email: string };
	cloudinary: {
		cloudName: string;
		apiKey: string;
		apiSecret: string;
		uploadPreset: string;
		connected: boolean;
	};
	preferences: {
		defaultDestination: RecordingSource;
		autoUpload: boolean;
	};
};

const defaultSettings: DashboardSettings = {
	profile: { name: "Kanak Kholwal", email: "kanak@perssonify.com" },
	cloudinary: {
		cloudName: "",
		apiKey: "",
		apiSecret: "",
		uploadPreset: "",
		connected: false,
	},
	preferences: { defaultDestination: "local", autoUpload: false },
};

class SettingsStore {
	value = $state<DashboardSettings>(defaultSettings);

	constructor() {
		const s = readJSON<Partial<DashboardSettings>>(SET_KEY, {});
		this.value = {
			profile: { ...defaultSettings.profile, ...s.profile },
			cloudinary: { ...defaultSettings.cloudinary, ...s.cloudinary },
			preferences: { ...defaultSettings.preferences, ...s.preferences },
		};
	}

	save() {
		if (browser) localStorage.setItem(SET_KEY, JSON.stringify(this.value));
	}

	get initials(): string {
		return (
			this.value.profile.name
				.split(/\s+/)
				.filter(Boolean)
				.slice(0, 2)
				.map((w) => w[0]!.toUpperCase())
				.join("") || "R"
		);
	}
}

export const recordingsStore = new RecordingsStore();
export const settingsStore = new SettingsStore();
