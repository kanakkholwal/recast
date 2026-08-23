import { safeStorage } from "@recast/ui/persisted-state";

/**
 * Dashboard data layer.
 *
 * Now backed by the real /api endpoints. `hydrate()` accepts server-loaded
 * recasts from the page loader and replaces the in-memory list. The
 * localStorage layer is retained only as an offline fallback — if a
 * reload happens while disconnected, the last cache shows instead of an
 * empty shell.
 */

export type RecordingSource = "cloud" | "local";

export type Recast = {
	id: string;
	title: string;
	durationSec: number;
	createdAt: number;
	sizeBytes: number;
	source: RecordingSource;
	provider: string | null;
	views: number;
	/** Owning folder, or null for the library root. */
	folderId: string | null;
	/** Tag ids — resolved against the workspace tag list in the UI. */
	tags: string[];
	/** Playable URL. May be a `blob:` URL for session uploads. */
	videoUrl: string;
	/** Poster image; empty string renders a gradient placeholder. */
	posterUrl: string;
	/** Slug of the recast's most recent share, or null if never shared. The
	 *  public link is `/share/{latestShareSlug}` — NOT `/share/{id}`. */
	latestShareSlug?: string | null;
};

/** Workspace storage quota used by the sidebar meter. */
export const STORAGE_QUOTA_BYTES = 5 * 1024 ** 3;

const REC_KEY = "recast.dashboard.recordings.v1";
// Pointer to the workspace whose recast cache is "current", so a cold load
// restores the right team's list instead of whichever was cached last.
const REC_WS_KEY = "recast.dashboard.recordings.ws";
const SET_KEY = "recast.dashboard.settings.v1";

/** Per-workspace cache key so switching teams never surfaces another team's
 *  recasts. Falls back to the legacy unscoped key when no workspace is known. */
function recKeyFor(workspaceId: string | null): string {
	return workspaceId ? `${REC_KEY}.${workspaceId}` : REC_KEY;
}

// Stable, public sample media so playback genuinely works on dummy data.
function sample(name: string) {
	return {
		videoUrl: `https://storage.googleapis.com/gtv-videos-bucket/sample/${name}.mp4`,
		posterUrl: `https://storage.googleapis.com/gtv-videos-bucket/sample/images/${name}.jpg`,
	};
}

const DAY = 86_400_000;

function seedRecordings(): Recast[] {
	const now = Date.now();
	return [
		{
			id: "rec_walkthrough",
			title: "Series A — product walkthrough",
			durationSec: 252,
			createdAt: now - 1 * DAY,
			sizeBytes: 191_000_000,
			source: "cloud",
			provider: "Cloudinary",
			views: 48,
			...sample("BigBuckBunny"),
		},
		{
			id: "rec_onboarding",
			title: "Onboarding flow v3",
			durationSec: 158,
			createdAt: now - 3 * DAY,
			sizeBytes: 101_000_000,
			source: "cloud",
			provider: "Cloudinary",
			views: 213,
			...sample("ElephantsDream"),
		},
		{
			id: "rec_changelog",
			title: "Changelog — sprint 22",
			durationSec: 64,
			createdAt: now - 4 * DAY,
			sizeBytes: 43_000_000,
			source: "local",
			provider: null,
			views: 0,
			...sample("ForBiggerBlazes"),
		},
		{
			id: "rec_bug",
			title: "Bug repro — export hang",
			durationSec: 52,
			createdAt: now - 6 * DAY,
			sizeBytes: 33_000_000,
			source: "local",
			provider: null,
			views: 0,
			...sample("ForBiggerEscapes"),
		},
		{
			id: "rec_teaser",
			title: "Launch teaser cut",
			durationSec: 31,
			createdAt: now - 9 * DAY,
			sizeBytes: 22_000_000,
			source: "cloud",
			provider: "Cloudinary",
			views: 1024,
			...sample("ForBiggerFun"),
		},
		{
			id: "rec_support",
			title: "Support reply — billing",
			durationSec: 107,
			createdAt: now - 13 * DAY,
			sizeBytes: 68_000_000,
			source: "local",
			provider: null,
			views: 0,
			...sample("ForBiggerJoyrides"),
		},
	].map((r) => ({ folderId: null, tags: [] as string[], ...r })) as Recast[];
}

/** Blob URLs don't survive a reload — fall back to sample media so the
 *  recording stays playable rather than becoming a dead entry. */
function reconcile(r: Recast): Recast {
	if (r.videoUrl?.startsWith("blob:")) {
		return { ...r, ...sample("WeAreGoingOnBubbles"), posterUrl: "" };
	}
	return r;
}

class RecordingsStore {
	items = $state<Recast[]>([]);
	hydrated = $state(false);
	// Which workspace the cached list belongs to. Read from the pointer so a
	// cold load restores that workspace's cache, not a stale other-team one.
	#workspaceId: string | null = safeStorage.get<string | null>(REC_WS_KEY, null);

	constructor() {
		const stored = safeStorage.get<Recast[] | null>(recKeyFor(this.#workspaceId), null);
		// Until `hydrate()` is called we show the last cached server list for
		// this workspace, or — if we've never seen one — the dummy seed so the
		// design surface stays explorable on logged-out previews.
		this.items = (stored ?? seedRecordings()).map(reconcile);
	}

	/**
	 * Point the cache at a workspace ahead of a cold load (call before the
	 * full-page reload on a team switch), so the next construct reads that
	 * team's scoped cache rather than the previous team's.
	 */
	hintWorkspace(workspaceId: string) {
		this.#workspaceId = workspaceId;
		safeStorage.set(REC_WS_KEY, workspaceId);
	}

	/**
	 * Replace the in-memory list with server-loaded rows. Persisted under the
	 * workspace-scoped key so the next cold load shows the same content
	 * instantly (then immediately revalidated by the next `hydrate()` call).
	 * Pass `workspaceId` on server-driven loads; omit it for in-place
	 * re-hydrations (optimistic rollbacks) that stay in the current scope.
	 */
	hydrate(server: Recast[], workspaceId?: string) {
		if (workspaceId) this.hintWorkspace(workspaceId);
		this.items = server;
		this.hydrated = true;
		this.persist();
	}

	private persist() {
		safeStorage.set(recKeyFor(this.#workspaceId), this.items);
	}

	get usedBytes(): number {
		return this.items.reduce((sum, r) => sum + r.sizeBytes, 0);
	}

	get cloudCount(): number {
		return this.items.filter((r) => r.source === "cloud").length;
	}

	add(rec: Recast) {
		this.items = [rec, ...this.items];
		this.persist();
	}

	remove(id: string) {
		this.items = this.items.filter((r) => r.id !== id);
		this.persist();
	}

	rename(id: string, title: string) {
		this.items = this.items.map((r) => (r.id === id ? { ...r, title } : r));
		this.persist();
	}

	setSource(id: string, source: RecordingSource) {
		this.items = this.items.map((r) =>
			r.id === id ? { ...r, source, provider: source === "cloud" ? "Cloudinary" : null } : r,
		);
		this.persist();
	}

	/** Move a recast to a folder (or null for root). Local mirror of the
	 *  PATCH /api/recasts/[id] call the caller makes. */
	move(id: string, folderId: string | null) {
		this.items = this.items.map((r) => (r.id === id ? { ...r, folderId } : r));
		this.persist();
	}

	/** Replace a recast's tag id set. Mirrors PUT /api/recasts/[id]/tags. */
	setTags(id: string, tags: string[]) {
		this.items = this.items.map((r) => (r.id === id ? { ...r, tags } : r));
		this.persist();
	}

	/** Cache a freshly-minted share slug so "Copy link" reuses it instead of
	 *  creating a duplicate share on the next click. */
	setShareSlug(id: string, slug: string) {
		this.items = this.items.map((r) => (r.id === id ? { ...r, latestShareSlug: slug } : r));
		this.persist();
	}

	/** Swap a recast's poster after a replace (mirrors PUT /api/recasts/[id]/poster). */
	setPoster(id: string, posterUrl: string) {
		this.items = this.items.map((r) => (r.id === id ? { ...r, posterUrl } : r));
		this.persist();
	}

	/** Strip a tag id from every recast that carried it (after the tag is
	 *  deleted server-side; the recast_tag rows cascade, this mirrors it locally). */
	removeTagEverywhere(tagId: string) {
		this.items = this.items.map((r) =>
			r.tags.includes(tagId) ? { ...r, tags: r.tags.filter((t) => t !== tagId) } : r,
		);
		this.persist();
	}

	/** Drop a folder reference from any recast that pointed at it (after the
	 *  folder — or its subtree — is deleted server-side; recasts fall to root). */
	clearFolder(folderIds: Set<string>) {
		this.items = this.items.map((r) =>
			r.folderId && folderIds.has(r.folderId) ? { ...r, folderId: null } : r,
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
		const s = safeStorage.get<Partial<DashboardSettings>>(SET_KEY, {});
		this.value = {
			profile: { ...defaultSettings.profile, ...s.profile },
			cloudinary: { ...defaultSettings.cloudinary, ...s.cloudinary },
			preferences: { ...defaultSettings.preferences, ...s.preferences },
		};
	}

	save() {
		safeStorage.set(SET_KEY, this.value);
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

/**
 * Workspace quota snapshot — usage vs plan caps. Hydrated by the
 * dashboard layout from `getQuotaSnapshot` on the server side. Reads
 * Infinity-coerced-to-null for unlimited caps (Enterprise tier).
 */
export type QuotaSnapshot = {
	plan: "free" | "pro" | "enterprise";
	usage: {
		storageBytes: number;
		activeRecastsCount: number;
		archivedRecastsCount: number;
		membersCount: number;
		deliveryBytesThisMonth: number;
	};
	limits: {
		storageBytes: number | null;
		activeRecasts: number | null;
		members: number | null;
		maxDurationSec: number | null;
		playbackMaxHeight: number;
		deliveryBytesPerMonth: number | null;
	};
	storagePctUsed: number;
	/** Bytes streamed to viewers this month — the metered infra cost. */
	delivery: {
		usedBytes: number;
		capBytes: number | null;
		ratio: number;
		exceeded: boolean;
		warn: boolean;
	};
};

class QuotaStore {
	value = $state<QuotaSnapshot | null>(null);

	hydrate(snap: QuotaSnapshot | null) {
		this.value = snap;
	}

	/** % of storage cap used; 0 when the workspace is unlimited. */
	get storagePct(): number {
		return this.value?.storagePctUsed ?? 0;
	}
}

export const recastsStore = new RecordingsStore();
export const settingsStore = new SettingsStore();
export const quotaStore = new QuotaStore();
