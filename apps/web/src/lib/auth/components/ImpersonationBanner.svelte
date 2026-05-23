<script lang="ts">
	import { authClient } from "$lib/auth/client";
	import { Button } from "@recast/ui/button";
	import { toast } from "@recast/ui/sonner";
	import { LoaderCircle, ShieldOff, UserCog } from "@lucide/svelte";
	import { cubicOut } from "svelte/easing";
	import { fly } from "svelte/transition";

	/**
	 * Global "you're impersonating someone" indicator. Mounted at the root
	 * layout so it surfaces on EVERY page (marketing, dashboard, admin, even
	 * the impersonated user's settings) — anywhere the admin might forget
	 * they're operating as another user.
	 *
	 * Pulls live session state via `authClient.useSession()` so the bar
	 * appears the moment the impersonation cookie is swapped, without a
	 * full page reload.
	 */

	const session = authClient.useSession();

	type SessionShape = {
		data: {
			session?: { impersonatedBy?: string | null } | null;
			user?: { email?: string | null; name?: string | null } | null;
		} | null;
	};

	const impersonatedBy = $derived(
		($session as unknown as SessionShape).data?.session?.impersonatedBy ?? null,
	);
	const targetEmail = $derived(
		($session as unknown as SessionShape).data?.user?.email ?? "user",
	);

	let stopping = $state(false);

	async function stop() {
		if (stopping) return;
		stopping = true;
		// Wrapped in try/finally so a thrown rejection (network drop, aborted
		// fetch) can't strand the button in a permanently-disabled state.
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.admin.stopImpersonating();
					if (error) throw new Error(error.message ?? "Couldn't stop impersonating.");
				})(),
				{
					loading: "Stopping impersonation…",
					success: "Back in your admin session.",
					error: (err) => (err as Error)?.message ?? "Couldn't stop impersonating.",
				},
			);
			// Hard reload so every server load re-runs against the restored
			// admin cookie. SvelteKit's `invalidateAll()` would leave the same
			// module instances around and we want a clean slate.
			window.location.href = "/admin";
		} finally {
			stopping = false;
		}
	}
</script>

{#if impersonatedBy}
	<div
		class="fixed inset-x-0 top-0 z-100 flex justify-center pointer-events-none"
		in:fly={{ y: -16, duration: 280, easing: cubicOut }}
		out:fly={{ y: -16, duration: 200, easing: cubicOut }}
	>
		<div
			role="status"
			aria-live="polite"
			class="pointer-events-auto m-3 flex max-w-[calc(100vw-1.5rem)] items-center gap-3 rounded-full border border-amber-500/35 bg-amber-500/15 px-3.5 py-2 text-[12px] font-medium text-amber-900 shadow-craft-floating backdrop-blur-md dark:text-amber-200"
		>
			<span class="grid size-6 shrink-0 place-items-center rounded-full bg-amber-500/25 text-amber-700 dark:text-amber-300">
				<UserCog class="size-3.5" />
			</span>
			<span class="hidden sm:inline">Impersonating</span>
			<span class="max-w-[40vw] truncate font-mono text-[11px] font-semibold sm:max-w-[28ch]">
				{targetEmail}
			</span>
			<Button
				size="sm"
				variant="outline"
				disabled={stopping}
				onclick={stop}
				class="ml-1 h-7 gap-1.5 rounded-full border-amber-500/40 bg-background/80 px-3 text-[11px] text-amber-900 hover:bg-background dark:text-amber-200"
			>
				{#if stopping}
					<LoaderCircle class="size-3 animate-spin" />
				{:else}
					<ShieldOff class="size-3" />
				{/if}
				{stopping ? "Stopping…" : "Stop"}
			</Button>
		</div>
	</div>
{/if}
