/**
 * Browser-export capability probe (migration Phase 0): can THIS WebView encode
 * H.264 through WebCodecs? WebView2 (Win) and modern WKWebView (macOS) can;
 * WebKitGTK (Linux) is the risk. The resolver ({@link chooseExportEngine}) routes
 * to the Rust compositor when this says no, so a machine without WebCodecs still
 * exports. Cached once per session — the answer can't change under the app.
 */

export interface BrowserExportCapability {
	/** WebCodecs H.264 encode is usable (hardware or software). */
	supported: boolean;
	/** A hardware encoder is available (vs. a software fallback). Telemetry only. */
	hardwareAccelerated: boolean;
	/** Why it's unsupported, for telemetry. */
	reason?: string;
}

// A baseline-AVC probe for whether this WebView does H.264 WebCodecs encode at all, not MediaBunny's exact profile.
const PROBE_CONFIG = {
	codec: "avc1.42001f",
	width: 1280,
	height: 720,
	bitrate: 4_000_000,
	framerate: 30,
} satisfies VideoEncoderConfig;

let cached: Promise<BrowserExportCapability> | null = null;

/** Probe (and cache) whether the browser export path can run here. */
export function probeBrowserExportCapability(): Promise<BrowserExportCapability> {
	if (!cached) cached = runProbe();
	return cached;
}

async function runProbe(): Promise<BrowserExportCapability> {
	if (typeof VideoEncoder === "undefined" || typeof VideoFrame === "undefined") {
		return { supported: false, hardwareAccelerated: false, reason: "webcodecs-unavailable" };
	}
	try {
		const hw = await VideoEncoder.isConfigSupported({
			...PROBE_CONFIG,
			hardwareAcceleration: "prefer-hardware",
		}).catch(() => null);
		if (hw?.supported) return { supported: true, hardwareAccelerated: true };
		const any = await VideoEncoder.isConfigSupported(PROBE_CONFIG).catch(() => null);
		if (any?.supported) return { supported: true, hardwareAccelerated: false };
		return { supported: false, hardwareAccelerated: false, reason: "h264-encode-unsupported" };
	} catch (e) {
		return { supported: false, hardwareAccelerated: false, reason: `probe-threw:${String(e)}` };
	}
}
