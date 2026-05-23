import type { Update } from "@tauri-apps/plugin-updater";
import { isTauriApp } from "$lib/runtime/tauri";

/**
 * Auto-updater store.
 *
 * Flow: on app boot we ask the Tauri updater plugin to compare the running
 * build against the `latest.json` manifest published with each GitHub release.
 * If a newer version exists we download it immediately in the background, then
 * surface a non-blocking corner card. The actual install + relaunch is
 * deferred until the user explicitly clicks "Restart to update".
 *
 * All updater/process APIs are imported lazily so the module is safe to load
 * in the browser (web build) where the Tauri plugins don't exist.
 */
export type UpdaterStatus =
	| "idle"
	| "checking"
	| "up-to-date"
	| "downloading"
	| "ready"
	| "error";

function createUpdaterStore() {
	let status = $state<UpdaterStatus>("idle");
	let version = $state<string | null>(null);
	let notes = $state<string | null>(null);
	let progress = $state(0); // 0..1, only meaningful while downloading
	let error = $state<string | null>(null);
	let dismissed = $state(false);
	let installing = $state(false);

	// The resolved Update handle. Held across the download → install steps.
	let update: Update | null = null;

	async function download() {
		if (!update) return;
		let total = 0;
		let received = 0;
		progress = 0;
		status = "downloading";
		try {
			await update.download((ev) => {
				switch (ev.event) {
					case "Started":
						total = ev.data.contentLength ?? 0;
						break;
					case "Progress":
						received += ev.data.chunkLength;
						progress = total > 0 ? Math.min(received / total, 1) : 0;
						break;
					case "Finished":
						progress = 1;
						break;
				}
			});
			status = "ready";
		} catch (e) {
			console.error("[updater] download failed", e);
			error = e instanceof Error ? e.message : String(e);
			status = "error";
		}
	}

	async function runCheck() {
		// Production-only. `tauri dev` ships an unsigned, unpublished build —
		// the updater plugin can't compare against `latest.json` in any
		// meaningful way, and surfacing the corner card during local
		// development just confuses contributors. Vite sets `import.meta.env.DEV`
		// from the running mode, so this short-circuits cleanly for
		// `tauri dev` while staying live for `tauri build` artefacts.
		if (import.meta.env.DEV) return;
		if (!(await isTauriApp())) return;
		if (status === "checking" || status === "downloading") return;
		error = null;
		status = "checking";
		try {
			const { check } = await import("@tauri-apps/plugin-updater");
			const found = await check();
			if (!found) {
				update = null;
				version = null;
				status = "up-to-date";
				return;
			}
			update = found;
			version = found.version;
			notes = found.body ?? null;
			dismissed = false;
			// Start pulling the new build straight away — the user only ever
			// waits on the install, never the download.
			await download();
		} catch (e) {
			console.error("[updater] check failed", e);
			error = e instanceof Error ? e.message : String(e);
			status = "error";
		}
	}

	return {
		get status() {
			return status;
		},
		get version() {
			return version;
		},
		get notes() {
			return notes;
		},
		get progress() {
			return progress;
		},
		get error() {
			return error;
		},
		get installing() {
			return installing;
		},

		/**
		 * Whether the corner card should render. We stay silent while idle,
		 * checking, or already up to date — the card only appears once there's
		 * something actionable (or a failure worth retrying).
		 */
		get visible() {
			if (dismissed) return false;
			return (
				status === "downloading" ||
				status === "ready" ||
				status === "error"
			);
		},

		/** Kick off the boot-time check. Fire-and-forget. */
		init() {
			void runCheck();
		},

		/** Re-run the check on demand (e.g. from settings / command palette). */
		checkNow() {
			return runCheck();
		},

		/** Hide the corner card until the next check finds something new. */
		dismiss() {
			dismissed = true;
		},

		/** Install the downloaded update and relaunch the app. */
		async installAndRelaunch() {
			if (!update || status !== "ready" || installing) return;
			installing = true;
			try {
				await update.install();
				const { relaunch } = await import("@tauri-apps/plugin-process");
				await relaunch();
			} catch (e) {
				console.error("[updater] install failed", e);
				error = e instanceof Error ? e.message : String(e);
				status = "error";
				installing = false;
			}
		},
	};
}

export const updater = createUpdaterStore();
