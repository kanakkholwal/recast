<script lang="ts">
/**
 * Manage an existing Recast Cloud share from the desktop: copy/open the link,
 * change who can view, set/remove a password, set/clear expiry (all via the
 * shared CloudShareSettings), or delete the cloud copy. Deleting NEVER touches
 * the local export. That stays the source of truth.
 */

import ConfirmDialog from "@recast/editor/components/dialog/ConfirmDialog.svelte";
import DialogShell from "@recast/editor/components/dialog/DialogShell.svelte";
import { Link2, Trash2 } from "@recast/icons";
import { Button } from "@recast/ui/button";
import { toast } from "@recast/ui/sonner";
import { type CloudUploadRecord } from "$lib/ipc";
import { cloudShare } from "$lib/stores/cloudShare.svelte";
import CloudShareSettings from "./CloudShareSettings.svelte";

let {
	open = false,
	record,
	fileName,
	path,
	onOpenChange,
}: {
	open?: boolean;
	record: CloudUploadRecord;
	fileName: string;
	path: string;
	onOpenChange?: (open: boolean) => void;
} = $props();

function close() {
	onOpenChange?.(false);
}

let save = $state<() => Promise<boolean>>(async () => true);
let saving = $state(false);
let loading = $state(true);
let deleting = $state(false);
// Deleting the cloud copy revokes a link other people may hold, so it asks first, like every destructive action.
let confirmDelete = $state(false);

async function onSave() {
	if (await save()) close();
}

async function deleteCloudCopy() {
	deleting = true;
	try {
		await cloudShare.deleteCloud(record.recastId, path);
		toast.success("Cloud copy deleted. Your local file is untouched.");
		close();
	} catch (e) {
		toast.error(`Couldn't delete: ${(e as Error)?.message ?? e}`);
	} finally {
		deleting = false;
	}
}
</script>

<DialogShell
	{open}
	title="Manage share"
	subtitle={fileName}
	icon={Link2}
	widthClass="sm:max-w-lg"
	onOpenChange={(v) => onOpenChange?.(v)}
>
	<CloudShareSettings
		recastId={record.recastId}
		slug={record.slug}
		shareUrl={record.shareUrl}
		bind:save
		bind:saving
		bind:loading
	/>

	{#snippet footer()}
		<Button
			type="button"
			variant="destructive_soft"
			size="xs"
			class="mr-auto gap-1.5"
			disabled={deleting || saving}
			onclick={() => (confirmDelete = true)}
		>
			<Trash2 class="size-3.5" />
			{deleting ? "Deleting…" : "Delete cloud copy"}
		</Button>
		<Button type="button" variant="ghost" size="xs" onclick={close}>Cancel</Button>
		<Button type="button" size="xs" disabled={saving || loading} onclick={onSave}>
			{saving ? "Saving…" : "Save"}
		</Button>
	{/snippet}
</DialogShell>

{#if confirmDelete}
	<ConfirmDialog
		open={true}
		title="Delete the cloud copy?"
		description="The share link stops working for everyone who has it. Your local file is untouched."
		confirmLabel="Delete cloud copy"
		variant="destructive"
		onConfirm={deleteCloudCopy}
		onOpenChange={(v) => {
			if (!v) confirmDelete = false;
		}}
	/>
{/if}
