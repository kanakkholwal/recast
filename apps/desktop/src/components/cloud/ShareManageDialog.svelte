<script lang="ts">
	/**
	 * Manage an existing Recast Cloud share from the desktop: copy/open the link,
	 * change who can view, set/remove a password, set/clear expiry (all via the
	 * shared CloudShareSettings), or delete the cloud copy. Deleting NEVER touches
	 * the local export. That stays the source of truth.
	 */
	import CloudShareSettings from "./CloudShareSettings.svelte";
	import { type CloudUploadRecord } from "$lib/ipc";
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import { Button } from "@recast/ui/button";
	import * as Dialog from "@recast/ui/dialog";
	import { toast } from "@recast/ui/sonner";
	import { Check, Link2, Trash2 } from "@lucide/svelte";

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

<Dialog.Root {open} onOpenChange={(v) => onOpenChange?.(v)}>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title class="flex items-center gap-2">
				<span
					class="grid size-7 place-items-center rounded-lg bg-primary/10 text-primary"
				>
					<Link2 class="size-3.5" />
				</span>
				Manage share
			</Dialog.Title>
			<Dialog.Description class="truncate">{fileName}</Dialog.Description>
		</Dialog.Header>

		<CloudShareSettings
			recastId={record.recastId}
			slug={record.slug}
			shareUrl={record.shareUrl}
			bind:save
			bind:saving
			bind:loading
		/>

		<Dialog.Footer class="gap-2">
			<Button
				type="button"
				variant="ghost"
				class="mr-auto gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
				disabled={deleting || saving}
				onclick={deleteCloudCopy}
			>
				<Trash2 class="size-3.5" />
				{deleting ? "Deleting…" : "Delete cloud copy"}
			</Button>
			<Button type="button" variant="ghost" onclick={close}>Cancel</Button>
			<Button
				type="button"
				disabled={saving || loading}
				class="gap-2"
				onclick={onSave}
			>
				{saving ? "Saving…" : "Save"}
				{#if !saving}<Check class="size-4" />{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
