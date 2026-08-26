import { toast } from "@recast/ui/sonner";
import type { SubmitFunction } from "@sveltejs/kit";
import { invalidateAll } from "$app/navigation";

export type EnhanceActionParams = {
	/** Flip the caller's in-flight flag (start true, finally false). */
	setBusy: (busy: boolean) => void;
	/** Toast shown when the action succeeds. */
	onSuccess?: string;
	/** Show `toast.error(result.data.error)` on validation failure. Actions that
	 *  never surface field errors pass false to stay silent. */
	onFailure?: boolean;
	/** Re-run every load (invalidateAll) after success, on top of update(). */
	invalidate?: boolean;
	/** Post-success side effect — clear a field, close a dialog. */
	reset?: () => void;
};

/**
 * Shared `use:enhance` callback for the admin form actions: toast + busy
 * tracking with an always-run `update()`. Bespoke flows (delete redirects,
 * impersonation) stay inline.
 */
export function enhanceAction(params: EnhanceActionParams): SubmitFunction {
	const { setBusy, onSuccess, onFailure = true, invalidate = false, reset } = params;
	return () => {
		setBusy(true);
		return async ({ result, update }) => {
			try {
				if (result.type === "success") {
					reset?.();
					if (onSuccess) toast.success(onSuccess);
					if (invalidate) await invalidateAll();
				} else if (result.type === "failure" && onFailure) {
					toast.error(String(result.data?.error));
				}
				await update();
			} finally {
				setBusy(false);
			}
		};
	};
}
