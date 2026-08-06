/**
 * Can this browser run the editor at all? Checked BEFORE a file is picked, so a
 * visitor on an unsupported browser is told up front instead of after choosing
 * a clip and waiting through a probe.
 */

import { hasVideoDecoder, hasVideoEncoder, hasWorkers } from "$lib/tools/capabilities";
import { isMobile } from "$lib/tools/device";

export type SupportLevel = "full" | "no-export" | "unsupported" | "mobile";

export interface SupportVerdict {
	level: SupportLevel;
	/** Null when nothing needs saying. */
	message: string | null;
	/** Whether the drop surface should still accept a file. */
	canEdit: boolean;
}

interface Probes {
	videoDecoder: boolean;
	videoEncoder: boolean;
	workers: boolean;
	webgl2: boolean;
	mobile: boolean;
}

/** Pure so the matrix is testable without a browser. */
export function verdictFrom(p: Probes): SupportVerdict {
	if (!p.workers || !p.videoDecoder || !p.webgl2) {
		return {
			level: "unsupported",
			canEdit: false,
			message:
				"This browser can't run the editor — it needs WebCodecs and WebGL2. Chrome, Edge or Arc will work, or use the desktop app.",
		};
	}
	// A phone can decode and composite, but a three-pane timeline editor on a
	// small touch screen is not the experience. Say so rather than let them find out.
	if (p.mobile) {
		return {
			level: "mobile",
			canEdit: true,
			message:
				"The editor is built for a large screen and a pointer. It'll run here, but it's much better on a desktop browser.",
		};
	}
	if (!p.videoEncoder) {
		return {
			level: "no-export",
			canEdit: true,
			message:
				"You can edit and preview here, but this browser can't encode video, so export is unavailable. The desktop app exports without it.",
		};
	}
	return { level: "full", canEdit: true, message: null };
}

function hasWebGL2(): boolean {
	if (typeof document === "undefined") return false;
	try {
		return document.createElement("canvas").getContext("webgl2") !== null;
	} catch {
		return false;
	}
}

export function checkSupport(): SupportVerdict {
	return verdictFrom({
		videoDecoder: hasVideoDecoder(),
		videoEncoder: hasVideoEncoder(),
		workers: hasWorkers(),
		webgl2: hasWebGL2(),
		mobile: isMobile(),
	});
}
