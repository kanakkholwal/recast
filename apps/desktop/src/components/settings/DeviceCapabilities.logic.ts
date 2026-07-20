/** Pure OS/encoder derivation + copy for DeviceCapabilities. */
import type { EncoderAvailability } from "$lib/ipc-types";

export const PLATFORM_LABEL: Record<string, string> = {
	windows: "Windows",
	macos: "macOS",
	linux: "Linux",
	ios: "iOS",
	android: "Android",
};

// Windows 11 still reports NT kernel 10.0; only build ≥22000 distinguishes it
// from 10, so we surface the build instead of the bare "10.0.26200".
export function windowsBuild(v: string): number | null {
	const m = /^\d+\.\d+\.(\d+)/.exec(v);
	return m ? Number(m[1]) : null;
}

export function deriveOsName(
	platform: string,
	osVersion: string,
	osLabel: string,
): string {
	if (platform === "windows") {
		const build = windowsBuild(osVersion);
		if (build !== null) return build >= 22000 ? "Windows 11" : "Windows 10";
	}
	if (platform === "macos" && osVersion) return `macOS ${osVersion}`;
	return osLabel;
}

// Build on Windows (the meaningful number), raw version elsewhere.
export function deriveOsDetail(platform: string, osVersion: string): string {
	if (platform === "windows") {
		const build = windowsBuild(osVersion);
		return build !== null ? String(build) : osVersion;
	}
	return osVersion;
}

export function captureHeadlineNote(
	screenNote: string | null | undefined,
	captureReady: boolean,
): string {
	if (screenNote) return screenNote;
	return captureReady
		? "Recast can record your whole screen, a single window, or a selected region on this device."
		: "Screen recording isn't available on this device yet. Editing, sharing, and playback still work.";
}

export function buildFacts(
	platform: string,
	osName: string,
	osDetail: string,
	osArch: string,
	ffmpegVersion: string | null | undefined,
): { label: string; value: string }[] {
	return [
		{ label: "Operating system", value: osName },
		{ label: platform === "windows" ? "Build" : "Version", value: osDetail },
		{ label: "Architecture", value: osArch },
		{
			label: "FFmpeg",
			value: ffmpegVersion?.replace(/^ffmpeg version\s*/i, "") ?? "Detecting…",
		},
	].filter((f) => f.value);
}

// Group encoders by codec family for the matrix; order follows first-appearance.
export function groupEncoders(
	encoders: EncoderAvailability[],
): { family: string; items: EncoderAvailability[] }[] {
	const groups: { family: string; items: EncoderAvailability[] }[] = [];
	for (const enc of encoders) {
		let group = groups.find((g) => g.family === enc.family);
		if (!group) {
			group = { family: enc.family, items: [] };
			groups.push(group);
		}
		group.items.push(enc);
	}
	return groups;
}
