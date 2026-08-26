// Capture-capability gating: caches the `capture_capabilities` probe and turns an
// unsupported feature into a ready-to-toast verdict, worded by reason:
//   - `unsupported` → OS / native API can't do it → "not supported on <os>"
//   - `planned`     → not shipped here yet → "not available yet"
//
// Fails OPEN: probe errors or unknown keys verdict `ok`, so a diagnostic hiccup
// never blocks a feature that might actually work.

import { type CaptureCapabilities, captureCapabilities } from "$lib/ipc";

let cached: Promise<CaptureCapabilities> | null = null;

/** Load (and cache) the capture-support matrix. Pass `force` to re-probe. */
export function loadCapabilities(force = false): Promise<CaptureCapabilities> {
	if (force || !cached) cached = captureCapabilities();
	return cached;
}

export type CapabilityVerdict =
	| { ok: true }
	| { ok: false; reason: "unsupported" | "planned"; message: string };

const OS_LABEL: Record<string, string> = {
	windows: "Windows",
	macos: "macOS",
	linux: "Linux",
};

/**
 * Decide whether `key` (a `CaptureCapability.key`) is usable on this device.
 * On `ok: false` the `message` is a finished, user-facing sentence ready to
 * drop into a toast. The caller just picks the surface.
 */
export async function checkCapability(
	key: string,
	fallbackLabel = "This feature",
): Promise<CapabilityVerdict> {
	let caps: CaptureCapabilities;
	try {
		caps = await loadCapabilities();
	} catch {
		// Probe failed, so let them try and surface the real error at use time.
		return { ok: true };
	}

	const capability = caps.capabilities.find((c) => c.key === key);
	// Unknown key, or genuinely supported → allow.
	if (!capability || capability.supported || capability.status === "supported") {
		return { ok: true };
	}

	const label = capability.label || fallbackLabel;
	const os = OS_LABEL[caps.platform] ?? "this system";

	if (capability.status === "planned") {
		return {
			ok: false,
			reason: "planned",
			message: capability.note
				? `${label} isn't available yet: ${capability.note}`
				: `${label} isn't available yet. It's coming in a future update.`,
		};
	}

	return {
		ok: false,
		reason: "unsupported",
		message: capability.note
			? `${label} isn't supported on ${os}: ${capability.note}`
			: `${label} isn't supported on ${os}.`,
	};
}
