/**
 * The playground's in-memory session: the picked source (and optional camera
 * clip), their object URLs, and the probed metadata.
 *
 * Deliberately ephemeral — nothing is persisted. `/playground` fills this in and
 * swaps its drop surface for the editor; a reload lands back on the picker.
 */

import type { MediaRef } from "@recast/media";

export interface PlaygroundMedia {
	file: File;
	/** For the `<video>` fallback element only; decode streams off the File. */
	objectUrl: string;
}

export interface PlaygroundMetadata {
	duration: number;
	width: number;
	height: number;
	fps: number;
}

class PlaygroundSession {
	source = $state<PlaygroundMedia | null>(null);
	camera = $state<PlaygroundMedia | null>(null);
	metadata = $state<PlaygroundMetadata | null>(null);
	/** True once an edit has been made, so we can warn before a reload. */
	dirty = $state(false);

	get ready(): boolean {
		return this.source !== null && this.metadata !== null;
	}

	/** Decode streams off the File; an object URL would risk a whole-file fetch. */
	get videoRef(): MediaRef | null {
		return this.source ? { kind: "blob", blob: this.source.file } : null;
	}

	get cameraRef(): MediaRef | null {
		return this.camera ? { kind: "blob", blob: this.camera.file } : null;
	}

	setSource(file: File, metadata: PlaygroundMetadata): void {
		this.#revoke(this.source);
		this.source = { file, objectUrl: URL.createObjectURL(file) };
		this.metadata = metadata;
		this.dirty = false;
	}

	setCamera(file: File | null): void {
		this.#revoke(this.camera);
		this.camera = file ? { file, objectUrl: URL.createObjectURL(file) } : null;
	}

	/**
	 * Drop everything and free the object URLs. Only call when leaving the
	 * playground — revoking while a decode worker holds a read in flight kills
	 * the decoder silently.
	 */
	clear(): void {
		this.#revoke(this.source);
		this.#revoke(this.camera);
		this.source = null;
		this.camera = null;
		this.metadata = null;
		this.dirty = false;
	}

	#revoke(media: PlaygroundMedia | null): void {
		if (media) URL.revokeObjectURL(media.objectUrl);
	}
}

export const playgroundSession = new PlaygroundSession();
