<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import { Button } from "@recast/ui/button";
import { Spinner } from "@recast/ui/spinner";
// Type-only: a value import here would statically bind the whole editor and
// defeat the split below (rolldown reports INEFFECTIVE_DYNAMIC_IMPORT).
import type { Editor as EditorComponent, EditorStore, PanelTab } from "@recast/editor";
import { webEditorServices } from "$lib/playground/services";
import { playgroundSession } from "$lib/playground/session.svelte";

// Code-split: the editor is tens of thousands of lines and must never land in
// the landing page's bundle.
let Editor = $state<typeof EditorComponent | null>(null);
let store = $state<EditorStore | null>(null);
let panels = $state<readonly PanelTab[]>([]);
let loadError = $state(false);

onMount(() => {
	if (!playgroundSession.ready) {
		void goto("/playground", { replaceState: true });
		return;
	}
	void import("@recast/editor")
		.then((m) => {
			const next = m.createEditorStore();
			const meta = playgroundSession.metadata!;
			next.metadata = { ...meta, codec: "", sizeBytes: playgroundSession.source!.file.size };
			next.loadRenderState({});
			store = next;
			panels = m.WEB_PANEL_TABS;
			Editor = m.Editor;
		})
		.catch(() => (loadError = true));
});

// Only warn once there is work to lose: an untouched clip is no loss.
function beforeUnload(event: BeforeUnloadEvent) {
	if (!playgroundSession.dirty) return;
	event.preventDefault();
}

const audioTracks = $derived(
	playgroundSession.videoRef
		? [{ src: playgroundSession.videoRef, kind: "system" as const }]
		: undefined,
);
</script>

<svelte:window onbeforeunload={beforeUnload} />

<svelte:head>
	<title>Editing — Recast playground</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<div class="h-dvh w-full">
	{#if loadError}
		<div class="flex h-full flex-col items-center justify-center gap-4">
			<p class="text-muted-foreground">The editor failed to load.</p>
			<Button variant="secondary" onclick={() => location.reload()}>Try again</Button>
		</div>
	{:else if Editor && store && playgroundSession.ready}
		<Editor
			{store}
			services={webEditorServices}
			videoSrc={playgroundSession.source!.objectUrl}
			cameraSrc={playgroundSession.camera?.objectUrl ?? ""}
			{audioTracks}
			{panels}
			filename={playgroundSession.source!.file.name}
		/>
	{:else}
		<div class="flex h-full flex-col items-center justify-center gap-3">
			<Spinner />
			<p class="text-muted-foreground text-sm">Loading the editor…</p>
		</div>
	{/if}
</div>
