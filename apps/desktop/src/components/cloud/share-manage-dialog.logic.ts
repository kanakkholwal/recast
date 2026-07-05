/** Pure share-update diff builder + visibility normalizer for ShareManageDialog. */

export type Visibility = "public" | "workspace" | "private";

export function toVisibility(v: string): Visibility {
	if (v === "public") return "public";
	if (v === "workspace" || v === "team") return "workspace";
	return "private";
}

export interface ShareUpdateInput {
	visibility: Visibility;
	initialVisibility: Visibility;
	removePassword: boolean;
	password: string;
	expiryDate: string; // yyyy-mm-dd, "" clears
	initialExpiry: string;
}

export interface ShareUpdateOpts {
	visibility?: Visibility;
	password?: string;
	expiresAt?: string;
}

/**
 * Assemble only the changed fields for `updateShare`. Semantics that MUST hold:
 *  - password: "" removes it (removePassword), a trimmed value sets it, and an
 *    untouched blank field is omitted (server keeps the existing password).
 *  - expiry: only emitted when the date changed; a date becomes end-of-day ISO
 *    (local time), an empty date emits "" to clear.
 */
export function buildShareUpdate(input: ShareUpdateInput): ShareUpdateOpts {
	const { visibility, initialVisibility, removePassword, password, expiryDate, initialExpiry } =
		input;
	const opts: ShareUpdateOpts = {};
	if (visibility !== initialVisibility) opts.visibility = visibility;
	if (removePassword) opts.password = "";
	else if (password.trim()) opts.password = password.trim();
	if (expiryDate !== initialExpiry) {
		// End-of-day in local time, ISO. Empty clears.
		opts.expiresAt = expiryDate
			? new Date(`${expiryDate}T23:59:59`).toISOString()
			: "";
	}
	return opts;
}
