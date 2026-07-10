/**
 * Persistent anonymous install id: the analytics `distinct_id` before sign-in.
 * Shares the `trace_install_id` localStorage key with `user-store` and the Rust
 * crash reporter so all three attribute to the same anonymous person.
 *
 * Standalone (no analytics/posthog imports) to avoid an import cycle with the
 * consent store and analytics client.
 */
import { safeStorage } from "@recast/ui/persisted-state";

const INSTALL_ID_KEY = "trace_install_id";

export function getInstallId(): string {
	// Stable sentinel for storage-less contexts (prerender / no window).
	if (typeof window === "undefined") return "anonymous-desktop";
	let id = safeStorage.get<string>(INSTALL_ID_KEY, "");
	if (!id) {
		id = crypto.randomUUID();
		// Best-effort persist; on quota/private-mode the ephemeral id is used.
		safeStorage.set(INSTALL_ID_KEY, id);
	}
	return id;
}
