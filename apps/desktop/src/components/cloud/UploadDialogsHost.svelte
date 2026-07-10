<script lang="ts">
	/**
	 * App-level host for the foreground upload dialogs (Recast Cloud share +
	 * Google Drive). Each renders whenever its store marks an upload as
	 * foregrounded, so uploads always surface in a dialog and can be reopened by
	 * clicking them in the activity center. Mounted once per shell.
	 */
	import CloudShareDialog from "./CloudShareDialog.svelte";
	import GdriveUploadDialog from "./GdriveUploadDialog.svelte";
	import { cloudShare } from "$lib/stores/cloudShare.svelte";
	import { gdrive } from "$lib/stores/gdrive.svelte";

	const sharePath = $derived(cloudShare.foregroundPath);
	const driveId = $derived(gdrive.foregroundId);
</script>

{#if sharePath}
	<CloudShareDialog path={sharePath} />
{/if}
{#if driveId}
	<GdriveUploadDialog uploadId={driveId} />
{/if}
