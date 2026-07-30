export type DestinationStatus = "idle" | "busy" | "done" | "error";

/** Terminal + in-flight states shared by the Recast Cloud and Drive stores. */
export type UploadPhase = "uploading" | "complete" | "error" | "cancelled";

export interface DestinationTile {
	status: DestinationStatus;
	label: string;
	/** Blocks a repeat click while the first one is still in flight. */
	disabled: boolean;
}

export interface DestinationLabels {
	idle: string;
	/** Shown once this path has landed at the destination. */
	done: string;
}

export interface DestinationInput {
	/** A pre-flight connection/sign-in check is running. */
	checking: boolean;
	/** Phase of the upload for this path, if one exists. */
	phase?: UploadPhase;
	/** A previous upload of this path is on record. */
	hasRecord: boolean;
}

/**
 * Button state for one upload destination. Deliberately status only, no percent
 * or bar: the foreground dialog and activity center own progress, this only has
 * to answer "did my click register, and can I click again?".
 */
export function destinationTile(
	labels: DestinationLabels,
	input: DestinationInput,
): DestinationTile {
	if (input.checking) return { status: "busy", label: "Checking…", disabled: true };

	switch (input.phase) {
		case "uploading":
			return { status: "busy", label: "Uploading…", disabled: true };
		case "error":
			return { status: "error", label: "Retry", disabled: false };
		case "complete":
			return { status: "done", label: labels.done, disabled: false };
		// A cancel returns the tile to idle: the point of cancelling is to be able
		// to start over, so it must not read as done or stay disabled.
		case "cancelled":
			return { status: "idle", label: labels.idle, disabled: false };
	}

	if (input.hasRecord) return { status: "done", label: labels.done, disabled: false };
	return { status: "idle", label: labels.idle, disabled: false };
}

/**
 * Newest upload for a path from a store keyed by upload id, preferring one still
 * in flight so a re-upload isn't masked by an older completed run.
 */
export function uploadForPath<T extends { sourcePath: string; status: UploadPhase }>(
	uploads: Record<string, T>,
	path: string,
): T | undefined {
	let latest: T | undefined;
	for (const upload of Object.values(uploads)) {
		if (upload.sourcePath !== path) continue;
		if (upload.status === "uploading") return upload;
		latest = upload;
	}
	return latest;
}
