<script lang="ts">
	import { invalidateAll } from "$app/navigation";
	import * as api from "$lib/dashboard/api";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { Input } from "@recast/ui/input";
	import { Label } from "@recast/ui/label";
	import { toast } from "@recast/ui/sonner";
	import { Textarea } from "@recast/ui/textarea";
	import { Check, LoaderCircle } from "@recast/icons";

	let {
		open = $bindable(false),
		recastId,
		title: initialTitle,
		description: initialDescription,
	}: {
		open?: boolean;
		recastId: string;
		title: string;
		description: string | null;
	} = $props();

	let title = $state("");
	let description = $state("");
	let saving = $state(false);

	// Seed the fields from the current values each time the dialog opens.
	$effect(() => {
		if (open) {
			title = initialTitle;
			description = initialDescription ?? "";
		}
	});

	const canSave = $derived(title.trim().length > 0 && !saving);

	async function save() {
		if (!canSave) return;
		saving = true;
		try {
			await api.updateRecastDetails(recastId, {
				title: title.trim(),
				description: description.trim(),
			});
			await invalidateAll();
			toast.success("Details saved.");
			open = false;
		} catch (e) {
			toast.error((e as Error)?.message ?? "Couldn't save the details.");
		} finally {
			saving = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Edit details</Dialog.Title>
			<Dialog.Description>Update this recast's title and description.</Dialog.Description>
		</Dialog.Header>

		<div class="space-y-4 py-1">
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Title</span>
				<Input bind:value={title} maxlength={200} placeholder="Untitled recast" class="h-10" />
			</Label>
			<Label class="block">
				<span class="mb-1 block text-xs font-semibold text-foreground/85">Description</span>
				<Textarea
					bind:value={description}
					maxlength={2000}
					placeholder="Add a description. Viewers see it on the share page."
					class="min-h-32 resize-y text-sm"
				/>
			</Label>
		</div>

		<div class="flex justify-end gap-2 pt-1">
			<Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
			<Button class="gap-2" disabled={!canSave} onclick={save}>
				{#if saving}
					<LoaderCircle class="size-4 animate-spin" />
				{:else}
					<Check class="size-4" />
				{/if}
				Save
			</Button>
		</div>
	</Dialog.Content>
</Dialog.Root>
