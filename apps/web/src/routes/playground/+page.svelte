<script lang="ts">
import { goto } from "$app/navigation";
import { Button } from "@recast/ui/button";
import { Download, Sparkles, Upload } from "@recast/icons";
import { ACCEPTED_EXTENSIONS, probeSource } from "$lib/playground/probe";
import { SAMPLE_CLIP } from "$lib/playground/sample";
import { playgroundSession } from "$lib/playground/session.svelte";

let dragging = $state(false);
let busy = $state(false);
let error = $state<string | null>(null);
let showDesktopCta = $state(false);
let fileInput = $state<HTMLInputElement | null>(null);

const accept = ACCEPTED_EXTENSIONS.map((e) => `.${e}`).join(",");

async function accepted(file: File) {
	busy = true;
	error = null;
	showDesktopCta = false;
	const result = await probeSource(file);
	if (!result.ok) {
		error = result.reason;
		showDesktopCta = result.suggestDesktop;
		busy = false;
		return;
	}
	const { ok: _ok, ...metadata } = result;
	playgroundSession.setSource(file, metadata);
	await goto("/playground/edit");
}

function onDrop(event: DragEvent) {
	event.preventDefault();
	dragging = false;
	const file = event.dataTransfer?.files?.[0];
	if (file) void accepted(file);
}

async function loadSample() {
	busy = true;
	error = null;
	try {
		const res = await fetch(SAMPLE_CLIP.src);
		if (!res.ok) throw new Error(`HTTP ${res.status}`);
		const blob = await res.blob();
		await accepted(new File([blob], SAMPLE_CLIP.filename, { type: "video/mp4" }));
	} catch {
		error = "The sample clip couldn't be loaded. Try dropping your own file.";
		busy = false;
	}
}
</script>

<svelte:head>
	<title>Video editor playground — edit a clip in your browser | Recast</title>
	<meta
		name="description"
		content="Try Recast's video editor in your browser. Drop in an MP4 or WebM and add backgrounds, zoom, annotations and captions, then export — no upload, no account. Your file never leaves your device."
	/>
</svelte:head>

<main class="mx-auto flex w-full max-w-3xl flex-col gap-10 px-6 py-20">
	<header class="flex flex-col gap-4 text-center">
		<h1 class="text-display-md font-semibold text-balance">The Recast editor, in your browser</h1>
		<p class="text-muted-foreground mx-auto max-w-xl text-pretty">
			Drop in a clip and try the real editor — backgrounds, zoom, annotations, captions and a
			proper timeline. Nothing is uploaded; your file is decoded on your own machine.
		</p>
	</header>

	<!-- The dropzone is a button so keyboard and screen-reader users get the same
	     affordance as a drag. -->
	<button
		type="button"
		class="border-border-low hover:border-primary/60 focus-visible:border-primary flex flex-col items-center gap-3 rounded-xl border-2 border-dashed px-6 py-16 transition-colors {dragging
			? 'border-primary bg-primary/5'
			: ''}"
		disabled={busy}
		onclick={() => fileInput?.click()}
		ondragover={(e) => {
			e.preventDefault();
			dragging = true;
		}}
		ondragleave={() => (dragging = false)}
		ondrop={onDrop}
	>
		<Upload class="text-muted-foreground size-8" />
		<span class="font-medium">{busy ? "Reading your clip…" : "Drop a video, or click to choose"}</span>
		<span class="text-muted-foreground text-sm">
			{ACCEPTED_EXTENSIONS.map((e) => e.toUpperCase()).join(" · ")}
		</span>
	</button>

	<input
		bind:this={fileInput}
		type="file"
		{accept}
		class="hidden"
		onchange={(e) => {
			const file = e.currentTarget.files?.[0];
			if (file) void accepted(file);
		}}
	/>

	<div class="flex flex-col items-center gap-3">
		<Button variant="secondary" disabled={busy} onclick={loadSample}>
			<Sparkles class="size-4" />
			Try a sample clip ({SAMPLE_CLIP.durationLabel} · {SAMPLE_CLIP.sizeLabel})
		</Button>
		{#if error}
			<p class="text-destructive max-w-lg text-center text-sm" role="alert">{error}</p>
			{#if showDesktopCta}
				<Button variant="outline" href="/download">
					<Download class="size-4" />
					Get the desktop app
				</Button>
			{/if}
		{/if}
	</div>

	<section class="text-muted-foreground grid gap-6 text-sm sm:grid-cols-3">
		<div>
			<h2 class="text-foreground mb-1 font-medium">Stays on your device</h2>
			<p>The clip is read straight off disk. Nothing is uploaded.</p>
		</div>
		<div>
			<h2 class="text-foreground mb-1 font-medium">The real editor</h2>
			<p>Same timeline, preview and export the desktop app ships.</p>
		</div>
		<div>
			<h2 class="text-foreground mb-1 font-medium">Recording lives in the app</h2>
			<p>Screen capture, camera and system audio need the desktop build.</p>
		</div>
	</section>
</main>
