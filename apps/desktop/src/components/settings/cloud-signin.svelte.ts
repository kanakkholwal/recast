/**
 * Recast Cloud sign-in state machine via the device-authorization flow.
 * States: loading | signed-out | waiting | signed-in | denied | expired.
 */

import { toast } from "@recast/ui/sonner";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
	type AuthPlan,
	type AuthStatus,
	type AuthUsage,
	authCancel,
	authSignOut,
	authStart,
	authStatus,
} from "$lib/ipc";
import { cloudShare } from "$lib/stores/cloudShare.svelte";

export type SignedInProfile = {
	email: string | null;
	name: string | null;
	image: string | null;
	memberSince: string | null;
	plan: AuthPlan | null;
	usage: AuthUsage | null;
};

export type ViewState =
	| { kind: "loading" }
	| { kind: "signed-out" }
	| {
			kind: "waiting";
			userCode: string;
			verificationUri: string;
			expiresAt: number;
	  }
	| ({ kind: "signed-in" } & SignedInProfile)
	| { kind: "denied" }
	| { kind: "expired" };

function toProfile(s: AuthStatus): SignedInProfile {
	return {
		email: s.email ?? null,
		name: s.name ?? null,
		image: s.image ?? null,
		memberSince: s.memberSince ?? null,
		plan: s.plan ?? null,
		usage: s.usage ?? null,
	};
}

export class CloudAuth {
	#view = $state<ViewState>({ kind: "loading" });
	// Which action is mid-flight, so the right button shows its own spinner. `null` = idle.
	#inFlight = $state<null | "sign-in" | "sign-out">(null);
	#unlisteners: UnlistenFn[] = [];
	#destroyed = false;

	get view(): ViewState {
		return this.#view;
	}
	get inFlight(): null | "sign-in" | "sign-out" {
		return this.#inFlight;
	}
	get busy(): boolean {
		return this.#inFlight !== null;
	}

	async loadStatus(): Promise<void> {
		try {
			const status = await authStatus();
			this.#view = status.signedIn
				? { kind: "signed-in", ...toProfile(status) }
				: { kind: "signed-out" };
			// Keep the shared store (read by the share flow) in sync.
			void cloudShare.refreshStatus();
		} catch (e) {
			toast.error(`Couldn't check sign-in state: ${e}`);
			this.#view = { kind: "signed-out" };
		}
	}

	async startSignIn(): Promise<void> {
		if (this.busy) return;
		this.#inFlight = "sign-in";
		try {
			const result = await authStart();
			this.#view = {
				kind: "waiting",
				userCode: result.user_code,
				verificationUri: result.verification_uri,
				expiresAt: Date.now() + result.expires_in * 1000,
			};
		} catch (e) {
			toast.error(`Couldn't start sign-in: ${e}`);
		} finally {
			this.#inFlight = null;
		}
	}

	async signOut(): Promise<void> {
		if (this.busy) return;
		this.#inFlight = "sign-out";
		try {
			await authSignOut();
			toast.success("Signed out of Recast Cloud.");
			this.#view = { kind: "signed-out" };
			// Drops the cached workspace list + persisted selection.
			void cloudShare.refreshStatus();
		} catch (e) {
			toast.error(`Couldn't sign out: ${e}`);
		} finally {
			this.#inFlight = null;
		}
	}

	/**
	 * Cancel an in-flight sign-in. Stops the Rust poller so an approval in the
	 * abandoned browser tab can't silently sign the user in.
	 */
	async cancelSignIn(): Promise<void> {
		this.#view = { kind: "signed-out" };
		try {
			await authCancel();
		} catch (e) {
			// Idempotent Rust-side; UI already reset, so don't surface.
			console.warn("auth_cancel failed (non-fatal):", e);
		}
	}

	/** Initial status check + subscribe to the Rust poller events. */
	start(): void {
		this.loadStatus();

		// Each handler ignores the firing if the view left "waiting". Defense in
		// depth over `auth_cancel`, in case a poll landed between Cancel and abort.
		void (async () => {
			const handles = await Promise.all([
				listen<AuthStatus>("auth:signed-in", (event) => {
					if (this.#view.kind !== "waiting") return;
					const s = event.payload;
					// Payload already carries the full profile (plan + usage), no refetch.
					this.#view = {
						kind: "signed-in",
						...toProfile(s ?? ({} as AuthStatus)),
					};
					void cloudShare.refreshStatus();
					toast.success("Signed in to Recast Cloud.");
				}),
				listen("auth:denied", () => {
					if (this.#view.kind !== "waiting") return;
					this.#view = { kind: "denied" };
				}),
				listen("auth:expired", () => {
					if (this.#view.kind !== "waiting") return;
					this.#view = { kind: "expired" };
				}),
				listen<string>("auth:error", (event) => {
					if (this.#view.kind !== "waiting") return;
					toast.error(`Sign-in error: ${event.payload}`);
					this.#view = { kind: "signed-out" };
				}),
				// Self-host endpoint changed: Rust dropped the old token, so re-check
				// against the new endpoint without a reload.
				listen("cloud:endpoint-changed", () => {
					if (this.#view.kind === "waiting") void this.cancelSignIn();
					this.#view = { kind: "loading" };
					this.loadStatus();
				}),
			]);
			// dispose() may have run while the listens were resolving, so release now.
			if (this.#destroyed) {
				for (const un of handles) un();
				return;
			}
			this.#unlisteners = handles;
		})();
	}

	/** Tear down the event subscriptions. */
	dispose(): void {
		this.#destroyed = true;
		for (const un of this.#unlisteners) un();
		this.#unlisteners = [];
	}
}
