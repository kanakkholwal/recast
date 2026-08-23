<script lang="ts">
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";
import { Input } from "@recast/ui/input";
import { Label } from "@recast/ui/label";
import { untrack } from "svelte";
import type { Recast } from "$lib/dashboard/store.svelte";

let {
	recast,
	onclose,
	onsave,
}: {
	recast: Recast;
	onclose: () => void;
	onsave: (title: string) => void;
} = $props();

// Seed once — the dialog is freshly mounted per rename, so no need to react.
let value = $state(untrack(() => recast.title));
const dirty = $derived(value.trim() !== "" && value.trim() !== recast.title);

function submit(e: SubmitEvent) {
	e.preventDefault();
	const title = value.trim();
	if (title && title !== recast.title) onsave(title);
	else onclose();
}
</script>

<Dialog.Root
	open
	onOpenChange={(next) => {
		if (!next) onclose();
	}}
>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Rename recast</Dialog.Title>
			<Dialog.Description>
				The title shows on the share page and in every link preview.
			</Dialog.Description>
		</Dialog.Header>
		<form onsubmit={submit} class="space-y-4">
			<Label class="block">
				<span class="mb-1.5 block text-body-sm font-medium text-foreground">Title</span>
				<!-- svelte-ignore a11y_autofocus -->
				<Input bind:value autofocus required class="h-9 border-border-low bg-background" />
			</Label>
			<Dialog.Footer>
				<Button type="button" variant="outline" size="sm" onclick={onclose}>Cancel</Button>
				<Button type="submit" size="sm" variant="dark" disabled={!dirty}>Save</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
