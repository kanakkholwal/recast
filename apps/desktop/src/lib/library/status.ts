/**
 * Which of the five states a library listing (recordings, exports) is in.
 *
 * Both pages used to branch on `isLoading && entries.length === 0` then fall
 * through to a single "nothing here" block, which told two lies: an empty list
 * during the first load rendered "No recordings yet", and a failed scan rendered
 * the same thing, so a backend error was indistinguishable from an empty disk.
 */
export type LibraryStatus = "loading" | "error" | "empty" | "no-matches" | "ready";

export interface LibraryStatusInput {
	loading: boolean;
	/** Message from a failed scan, or null. */
	error: string | null;
	/** Items before the search filter. */
	total: number;
	/** Items after the search filter. */
	matches: number;
	query: string;
}

export function libraryStatus({
	loading,
	error,
	total,
	matches,
	query,
}: LibraryStatusInput): LibraryStatus {
	// A stale list beats a skeleton on refresh, so only a first load shows one.
	if (loading && total === 0) return "loading";
	if (error && total === 0) return "error";
	if (total === 0) return "empty";
	if (matches === 0) return query.trim() ? "no-matches" : "empty";
	return "ready";
}

/** True once the listing can honestly state how many items it holds. */
export function canReportCount(status: LibraryStatus): boolean {
	return status !== "loading" && status !== "error";
}
