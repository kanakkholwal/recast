/** Camera/device id predicates shared by the panel and camera-preview windows. */

/**
 * True for a browser MediaDevices id (a long hex hash) as opposed to a legacy
 * DirectShow friendly name. The Rust validator only understands DirectShow
 * names, so hash ids skip native validation and go straight through
 * `openCameraStream`.
 */
export function isBrowserDeviceId(id: string): boolean {
	return /^[a-f0-9]{40,}$/i.test(id);
}
