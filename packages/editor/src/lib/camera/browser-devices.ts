// Rust enumerates DirectShow names but getUserMedia takes MediaDevices ids; passing the wrong one lets the browser pick its own default.

const VIRTUAL_CAMERA_PATTERNS: RegExp[] = [
	/phone\s*link/i,
	/windows\s*camera/i,
	/obs\s*virtual/i,
	/obs-?camera/i,
	/nvidia\s*broadcast/i,
	/snap\s*camera/i,
	/xsplit/i,
	/manycam/i,
	/e2esoft/i,
	/splitcam/i,
	/droidcam/i,
	/iriun/i,
	/epoccam/i,
];

export interface BrowserCamera {
	deviceId: string;
	label: string;
	groupId: string;
	isVirtual: boolean;
}

export function isVirtualCameraLabel(label: string): boolean {
	return VIRTUAL_CAMERA_PATTERNS.some((p) => p.test(label));
}

/**
 * Why enumeration couldn't produce a usable device. Only the *blocker* cases:
 *   - `unavailable` → no MediaDevices API at all (macOS WKWebView without
 *     NSCameraUsageDescription, or Linux WebKitGTK with media-stream off).
 *   - `denied` → a camera exists but capture was refused.
 * An empty result with NO error means genuinely no camera connected; callers
 * distinguish that case so the UI can say the right thing.
 */
export type CameraAccessReason = "unavailable" | "denied";

export class CameraAccessError extends Error {
	readonly reason: CameraAccessReason;
	constructor(reason: CameraAccessReason, message: string) {
		super(message);
		this.name = "CameraAccessError";
		this.reason = reason;
	}
}

/** A getUserMedia rejection that means "blocked", not "device busy" / other. */
function isPermissionDenied(e: unknown): boolean {
	return e instanceof DOMException && (e.name === "NotAllowedError" || e.name === "SecurityError");
}

/**
 * Guard against the WebView stripping `navigator.mediaDevices` entirely (macOS
 * WKWebView without NSCameraUsageDescription; Linux WebKitGTK with media-stream
 * off). Without this the API throws an opaque "undefined is not an object"
 * instead of an actionable error.
 */
function assertMediaDevices(): MediaDevices {
	const media = navigator.mediaDevices;
	if (!media || typeof media.enumerateDevices !== "function") {
		throw new CameraAccessError(
			"unavailable",
			"Camera access isn't available in this build. Update Recast, then " +
				"check that camera permission is enabled for it in your system settings.",
		);
	}
	return media;
}

/**
 * Enumerate video input devices visible to this WebView. Triggers a one-shot
 * permission probe if labels are blank (browsers strip labels until permission
 * is granted at least once). Real hardware is sorted ahead of virtual cameras
 * so callers that pick `[0]` get a sane default.
 */
export async function enumerateCameras(): Promise<BrowserCamera[]> {
	const media = assertMediaDevices();
	let devices = await media.enumerateDevices();
	// No videoinput means no camera: return empty rather than probing and popping a needless prompt.
	if (!devices.some((d) => d.kind === "videoinput")) return [];

	const labelsPopulated = devices.some((d) => d.kind === "videoinput" && Boolean(d.label));
	if (!labelsPopulated) {
		// Labels stay blank until capture is authorized once; probing unlocks them and surfaces a silent block.
		try {
			const probe = await media.getUserMedia({ video: true });
			for (const t of probe.getTracks()) t.stop();
		} catch (e) {
			if (isPermissionDenied(e)) {
				throw new CameraAccessError(
					"denied",
					"Camera access is blocked. Allow it in your system settings, then rescan.",
				);
			}
			// Device-busy and friends are non-fatal: keep the unlabeled device.
			console.warn("[camera] label probe failed:", e);
		}
		devices = await media.enumerateDevices();
	}

	return devices
		.filter((d) => d.kind === "videoinput")
		.map((d) => ({
			deviceId: d.deviceId,
			label: d.label || "Camera",
			groupId: d.groupId,
			isVirtual: isVirtualCameraLabel(d.label),
		}))
		.sort((a, b) => Number(a.isVirtual) - Number(b.isVirtual));
}

/**
 * Resolve a query (browser deviceId, exact label, or DirectShow name) to a
 * specific camera. Falls back to fuzzy label matching, but always prefers
 * non-virtual hardware when multiple candidates match.
 */
export function findCamera(
	cameras: BrowserCamera[],
	query: string | null | undefined,
): BrowserCamera | null {
	if (!query) return null;
	const direct = cameras.find((c) => c.deviceId === query);
	if (direct) return direct;
	const exact = cameras.find((c) => c.label === query);
	if (exact) return exact;
	const norm = (s: string) => s.toLowerCase().replace(/\s+/g, " ").trim();
	const q = norm(query);
	const partial = cameras.filter((c) => {
		const lbl = norm(c.label);
		return lbl.includes(q) || q.includes(lbl);
	});
	if (partial.length === 0) return null;
	return partial.find((c) => !c.isVirtual) ?? partial[0];
}

export class CameraNotFoundError extends Error {
	readonly query: string | null;
	constructor(query: string | null, message: string) {
		super(message);
		this.name = "CameraNotFoundError";
		this.query = query;
	}
}

/**
 * Open a stream for `query` (or the best non-virtual default if null). Always
 * uses `deviceId: { exact }` so the browser cannot substitute another device.
 * Throws CameraNotFoundError instead of silently picking a default.
 */
export async function openCameraStream(
	query: string | null,
): Promise<{ stream: MediaStream; camera: BrowserCamera }> {
	const cameras = await enumerateCameras();
	if (cameras.length === 0) {
		throw new CameraNotFoundError(query, "No camera devices available.");
	}

	const target = query
		? findCamera(cameras, query)
		: (cameras.find((c) => !c.isVirtual) ?? cameras[0]);

	if (!target) {
		throw new CameraNotFoundError(
			query,
			`Requested camera "${query}" is not available in this WebView.`,
		);
	}

	const stream = await assertMediaDevices().getUserMedia({
		video: { deviceId: { exact: target.deviceId } },
		audio: false,
	});
	return { stream, camera: target };
}
