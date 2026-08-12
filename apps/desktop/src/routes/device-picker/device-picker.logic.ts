/** URL params + device mapping/defaulting for the device-picker window. */

import type { BrowserCamera } from "@recast/editor/lib/camera/browser-devices";
import type { AudioDeviceInfo } from "@recast/editor/lib/wire-types";
import type { CameraDeviceInfo } from "$lib/recorder-types";

export type DeviceType = "mic" | "camera";

export function parseDevicePickerParams(search: string): {
	deviceType: DeviceType;
	selectedId: string | null;
} {
	const params = new URLSearchParams(search);
	return {
		deviceType: params.get("type") === "camera" ? "camera" : "mic",
		selectedId: params.get("selected") ?? null,
	};
}

/**
 * Browser cameras → picker rows. Virtual cameras carry a warning status so the
 * list flags them; a real webcam is preferred by `pickDefault` (non-virtual
 * first is guaranteed by the enumeration).
 */
export function mapCameras(cams: BrowserCamera[]): CameraDeviceInfo[] {
	return cams.map((c) => ({
		id: c.deviceId,
		name: c.label,
		status: c.isVirtual ? "warning" : "ready",
		statusMessage: c.isVirtual ? "Virtual camera" : null,
	}));
}

/**
 * Default selection id for a fresh picker: the system-default mic, or the first
 * camera (enumeration already puts real webcams ahead of virtual ones). `null`
 * when the list is empty.
 */
export function pickDefault(
	devices: (AudioDeviceInfo | CameraDeviceInfo)[],
	isMic: boolean,
): string | null {
	if (devices.length === 0) return null;
	const def = isMic ? (devices as AudioDeviceInfo[]).find((d) => d.isDefault) : devices[0];
	return def?.id ?? null;
}
