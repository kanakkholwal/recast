// Browser-side camera enumeration. The Rust ffmpeg/dshow enumeration returns
// DirectShow friendly names, but the WebView's getUserMedia operates on
// MediaDevices deviceIds. When those disagree (e.g., Phone Link or other
// virtual cameras registered ahead of the real webcam), passing a DirectShow
// name to getUserMedia silently fails and falling back to `video: true` lets
// the browser pick its own default — which on Windows is often Phone Link.
//
// Use this module everywhere the WebView needs to open a specific camera.

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
 * Enumerate video input devices visible to this WebView. Triggers a one-shot
 * permission probe if labels are blank (browsers strip labels until permission
 * is granted at least once). Real hardware is sorted ahead of virtual cameras
 * so callers that pick `[0]` get a sane default.
 */
export async function enumerateCameras(): Promise<BrowserCamera[]> {
	let devices = await navigator.mediaDevices.enumerateDevices();
	const labelsPopulated = devices.some(
		(d) => d.kind === "videoinput" && !!d.label,
	);
	if (!labelsPopulated) {
		try {
			const probe = await navigator.mediaDevices.getUserMedia({ video: true });
			probe.getTracks().forEach((t) => t.stop());
		} catch (e) {
			console.warn("[camera] label probe failed:", e);
		}
		devices = await navigator.mediaDevices.enumerateDevices();
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

	const stream = await navigator.mediaDevices.getUserMedia({
		video: { deviceId: { exact: target.deviceId } },
		audio: false,
	});
	return { stream, camera: target };
}
