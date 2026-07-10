/**
 * Opt-in diagnostic-logging switch, shared across windows.
 *
 * Rust `AppConfig.diagnostic_logging` is the source of truth (it drives the
 * runtime log level via `apply_log_level`), so we adopt it on startup.
 * `PersistedState` is layered on for cross-window reactivity, so a Settings
 * toggle reaches an open editor window's logger without a reload.
 *
 * Off by default; when on, verbose logs land in the rotating log file.
 */

import { PersistedState } from "@recast/ui/persisted-state";
import { getDiagnosticLogging, setDiagnosticLogging } from "$lib/ipc";

const STORAGE_KEY = "recast-diagnostic-logging";

function createDiagnosticsStore() {
	const state = new PersistedState<boolean>(STORAGE_KEY, false);

	// Adopt the backend value so the toggle is correct even after localStorage
	// is cleared. Best-effort: non-Tauri previews keep the local/default value.
	void getDiagnosticLogging()
		.then((backend) => {
			if (typeof backend === "boolean") state.current = backend;
		})
		.catch(() => {});

	return {
		/** Reactive: read inside an `$effect`/`$derived` to track changes. */
		get enabled() {
			return state.current;
		},
		/** Persist the choice to localStorage (cross-window) AND Rust (log level). */
		set(value: boolean) {
			state.current = value;
			void setDiagnosticLogging(value).catch(() => {});
		},
	};
}

export const diagnostics = createDiagnosticsStore();
