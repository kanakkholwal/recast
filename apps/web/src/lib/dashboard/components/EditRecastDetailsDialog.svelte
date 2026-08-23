<script lang="ts">
import { Check, LoaderCircle } from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { toast } from "@recast/ui/sonner";
import { Textarea } from "@recast/ui/textarea";
import { invalidateAll } from "$app/navigation";
import * as api from "$lib/dashboard/api";

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
				<span class="mb-1.5 block text-body-sm font-medium text-foreground">Title</span>
				<Input
					bind:value={title}
					maxlength={200}
					placeholder="Untitled recast"
					class="h-9 border-border-low bg-background"
				/>
			</Label>
			<Label class="block">
				<span class="mb-1.5 block text-body-sm font-medium text-foreground">Description</span>
				<Textarea
					bind:value={description}
					maxlength={2000}
					placeholder="Add a description. Viewers see it on the share page."
					class="min-h-32 resize-y border-border-low bg-background text-body-sm"
				/>
			</Label>
		</div>

		<Dialog.Footer>
			<Button variant="outline" size="sm" onclick={() => (open = false)}>Cancel</Button>
			<Button size="sm" variant="dark" class="gap-2" disabled={!canSave} onclick={save}>
				{#if saving}
					<LoaderCircle class="size-3.5 animate-spin" />
				{:else}
					<Check class="size-3.5" />
				{/if}
				{saving ? "Saving…" : "Save"}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
