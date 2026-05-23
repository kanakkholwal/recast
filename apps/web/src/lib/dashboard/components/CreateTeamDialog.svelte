<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { ArrowRight, LoaderCircle, Plus } from "@lucide/svelte";

	/**
	 * Inline "create another team" flow for users who already have at least
	 * one team. /onboarding/team handles the zero-team case; this dialog
	 * handles every subsequent create.
	 *
	 * Slug is auto-derived from the name plus a 6-char random suffix so the
	 * unique index never collides.
	 */

	let { open = $bindable(false) }: { open?: boolean } = $props();

	let name = $state("");
	let creating = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!name.trim() || creating) return;
		creating = true;
		const teamName = name.trim();
		const slug = `${teamName
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/(^-|-$)/g, "") || "team"}-${Math.random().toString(36).slice(2, 8)}`;
		try {
			await toast.promise(
				(async () => {
					const { error } = await authClient.organization.create({
						name: teamName,
						slug,
						keepCurrentActiveOrganization: false,
					});
					if (error) {
						// Surface the real reason: cap reached, slug clash, etc.
						console.error("[create team]", error);
						throw new Error(error.message ?? "Couldn't create the team.");
					}
				})(),
				{
					loading: `Creating ${teamName}…`,
					success: `Welcome to ${teamName}.`,
					error: (err) => (err as Error)?.message ?? "Couldn't create the team.",
				},
			);
			name = "";
			open = false;
			// Active org has been switched server-side by setActive — re-pull
			// every loader so the sidebar swaps over to the new team.
			await invalidateAll();
		} finally {
			// Always release so a thrown rejection (network drop, abort) can't
			// strand the submit button in a disabled state.
			creating = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span class="glass-chip grid size-7 place-items-center rounded-lg text-primary">
					<Plus class="size-3.5" />
				</span>
				Create a team
			</Dialog.Title>
			<Dialog.Description>
				You'll be the owner. The team starts on the free plan with 3 seats —
				an admin can upgrade it later.
			</Dialog.Description>
		</Dialog.Header>
		<form class="space-y-3" onsubmit={submit}>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">
					Team name
				</span>
				<Input
					bind:value={name}
					placeholder="Acme demos"
					class="h-10"
					required
					autofocus
				/>
			</Label>
			<Dialog.Footer>
				<Button type="button" variant="ghost" onclick={() => (open = false)}>
					Cancel
				</Button>
				<Button type="submit" disabled={creating || !name.trim()} class="gap-2">
					{creating ? "Creating…" : "Create team"}
					{#if creating}
						<LoaderCircle class="size-4 animate-spin" />
					{:else}
						<ArrowRight class="size-4" />
					{/if}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
