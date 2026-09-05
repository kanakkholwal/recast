/**
 * Export preferences that outlived the experimental flags they started as.
 *
 * The engine export (the same compositor the preview draws with) is the default
 * path. `forceLegacy` is the support escape hatch `chooseExportEngine` was built
 * around: a machine whose WebView encodes badly needs a way back to the FFmpeg
 * compositor without waiting for a release.
 */

import { PersistedState } from "@recast/ui/persisted-state";

const STORAGE_KEY = "recast-export-preferences";

interface ExportPreferences {
	forceLegacy: boolean;
}

const DEFAULTS: ExportPreferences = { forceLegacy: false };

function createExportPreferences() {
	const prefs = new PersistedState<ExportPreferences>(STORAGE_KEY, DEFAULTS);
	return {
		get forceLegacy() {
			return prefs.current.forceLegacy;
		},
		setForceLegacy(value: boolean) {
			prefs.current = { ...prefs.current, forceLegacy: value };
		},
	};
}

export const exportPreferences = createExportPreferences();
