<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import { authClient } from "$lib/auth/client";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { ArrowRight, Plus } from "@lucide/svelte";

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
		const slug = `${name
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/(^-|-$)/g, "") || "team"}-${Math.random().toString(36).slice(2, 8)}`;
		const { error } = await authClient.organization.create({
			name: name.trim(),
			slug,
			keepCurrentActiveOrganization: false,
		});
		creating = false;
		if (error) {
			// Surface the real reason: cap reached, slug clash, etc.
			toast.error(error.message ?? "Couldn't create the team.");
			console.error("[create team]", error);
			return;
		}
		toast.success(`Welcome to ${name.trim()}.`);
		name = "";
		open = false;
		// Active org has been switched server-side by setActive — re-pull
		// every loader so the sidebar swaps over to the new team.
		await invalidateAll();
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
					<ArrowRight class="size-4" />
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
