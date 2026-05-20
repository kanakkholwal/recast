/**
 * Experimental-features flags, shared across the settings page and any
 * surface that gates a feature behind one. Persisted to localStorage so the
 * choice survives reload; off by default so first-run users don't see
 * unfinished UI.
 *
 * Add a new flag by extending `ExperimentalFlag`, adding it to `DEFAULTS`,
 * and exposing a getter/setter pair on the store. The settings page reads
 * the registry via `FLAG_META` to render a row per flag without manual
 * wiring.
 */

export type ExperimentalFlag = "silenceDetection";

interface FlagMeta {
	key: ExperimentalFlag;
	label: string;
	description: string;
}

export const FLAG_META: FlagMeta[] = [
	{
		key: "silenceDetection",
		label: "Silence detection & cuts",
		description:
			"Detect dead air (quiet audio + still cursor) and skip it during playback/export. Hidden when off.",
	},
];

const DEFAULTS: Record<ExperimentalFlag, boolean> = {
	silenceDetection: false,
};

const STORAGE_KEY = "recast-experimental-flags";

function load(): Record<ExperimentalFlag, boolean> {
	if (typeof localStorage === "undefined") return { ...DEFAULTS };
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return { ...DEFAULTS };
		const parsed = JSON.parse(raw) as Partial<Record<ExperimentalFlag, boolean>>;
		return { ...DEFAULTS, ...parsed };
	} catch {
		return { ...DEFAULTS };
	}
}

function persist(flags: Record<ExperimentalFlag, boolean>) {
	if (typeof localStorage === "undefined") return;
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(flags));
	} catch {
		// Quota / private mode — best effort.
	}
}

function createExperimentalStore() {
	// Read once at module init. localStorage is available in Tauri webviews;
	// the load() guard handles the SSR/no-window edge cases.
	let flags = $state<Record<ExperimentalFlag, boolean>>(load());

	// Cross-window sync. Tauri v2 webviews share localStorage origin, so a
	// write from the settings window fires a `storage` event in any editor
	// windows that were already open. Re-read on match so the flag flip is
	// reflected without a reload. Same-window writes don't fire `storage`,
	// but `setEnabled` updates the rune directly — both paths covered.
	if (typeof window !== "undefined") {
		window.addEventListener("storage", (e) => {
			if (e.key !== STORAGE_KEY) return;
			flags = load();
		});
	}

	return {
		get silenceDetection() {
			return flags.silenceDetection;
		},
		isEnabled(key: ExperimentalFlag): boolean {
			return flags[key];
		},
		setEnabled(key: ExperimentalFlag, value: boolean) {
			flags = { ...flags, [key]: value };
			persist(flags);
		},
	};
}

export const experimentalStore = createExperimentalStore();
