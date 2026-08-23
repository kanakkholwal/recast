<script lang="ts">
import { LoaderCircle } from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Dialog from "@recast/ui/dialog";

// Confirmation for actions that destroy something. Deleting a recast takes
// its shares, views and comments with it, and archiving drops the video
// file; neither is undoable, so neither should be one click from a menu.
let {
	open = $bindable(false),
	title,
	description,
	confirmLabel = "Delete",
	destructive = true,
	busy = false,
	onconfirm,
}: {
	open?: boolean;
	title: string;
	description: string;
	confirmLabel?: string;
	destructive?: boolean;
	busy?: boolean;
	onconfirm: () => void;
} = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{title}</Dialog.Title>
			<Dialog.Description>{description}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" size="sm" disabled={busy} onclick={() => (open = false)}>
				Cancel
			</Button>
			<Button
				variant={destructive ? "destructive" : "dark"}
				size="sm"
				disabled={busy}
				class="gap-2"
				onclick={onconfirm}
			>
				{#if busy}
					<LoaderCircle class="size-3.5 animate-spin" />
				{/if}
				{confirmLabel}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
