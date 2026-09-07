/**
 * Accept-or-explain gate for a dropped file: container/codec support, and a
 * duration ceiling derived from the device rather than a business rule.
 */

import { checkFileSize, deviceMemoryGB, inputBudget, isMobile } from "$lib/tools/device";
import type { PlaygroundMetadata } from "./session.svelte";

export const ACCEPTED_EXTENSIONS = ["mp4", "webm", "mov", "m4v"];

export interface ProbeRejection {
	ok: false;
	reason: string;
	/** True when the desktop app is the honest next step. */
	suggestDesktop: boolean;
}
export type ProbeResult = ({ ok: true } & PlaygroundMetadata) | ProbeRejection;

/**
 * Longest clip this device should attempt. Editing holds decoded frames, a
 * filmstrip and (on export) an encoder, so the ceiling tracks the same memory
 * signal the conversion tools budget against.
 */
export function maxDurationSec(deviceMemoryGb: number | null, mobile: boolean): number {
	if (mobile) return 60;
	const gb = deviceMemoryGb ?? 4;
	if (gb >= 16) return 30 * 60;
	if (gb >= 8) return 15 * 60;
	if (gb >= 4) return 8 * 60;
	return 4 * 60;
}

export function isAcceptedFile(file: File): boolean {
	const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
	return ACCEPTED_EXTENSIONS.includes(ext) || file.type.startsWith("video/");
}

export async function probeSource(file: File): Promise<ProbeResult> {
	if (!isAcceptedFile(file)) {
		return {
			ok: false,
			suggestDesktop: false,
			reason: `That doesn't look like a video. Try ${ACCEPTED_EXTENSIONS.join(", ")}.`,
		};
	}
	const size = checkFileSize(file.size, inputBudget());
	if (!size.ok) return { ok: false, suggestDesktop: true, reason: size.reason ?? "" };

	// Imported here, not at module scope: this pulls MediaBunny in, and the landing page stays light until a file drops.
	const { openInput } = await import("@recast/media");
	let input: Awaited<ReturnType<typeof openInput>> | null = null;
	try {
		input = await openInput(file);
		const track = await input.getPrimaryVideoTrack();
		if (!track) {
			return { ok: false, suggestDesktop: false, reason: "This file has no video track." };
		}
		if (!(await track.canDecode())) {
			const codec = await track.getCodec();
			return {
				ok: false,
				suggestDesktop: true,
				reason: `This browser can't decode ${codec ?? "this video's codec"}.`,
			};
		}
		const duration = await input.computeDuration();
		const limit = maxDurationSec(deviceMemoryGB(), isMobile());
		if (duration > limit) {
			return {
				ok: false,
				suggestDesktop: true,
				reason: `Clips up to ${Math.round(limit / 60)} minutes work well here; this one is ${Math.round(duration / 60)}.`,
			};
		}
		const stats = await track.computePacketStats(100).catch(() => null);
		return {
			ok: true,
			duration,
			width: await track.getCodedWidth(),
			height: await track.getCodedHeight(),
			fps: Math.round(stats?.averagePacketRate ?? 30) || 30,
		};
	} catch {
		return {
			ok: false,
			suggestDesktop: true,
			reason: "That file couldn't be read. It may be corrupt or use an unsupported container.",
		};
	} finally {
		input?.dispose();
	}
}
