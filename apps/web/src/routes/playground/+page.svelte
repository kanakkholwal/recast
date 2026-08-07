<script lang="ts">
import { onDestroy, onMount, untrack } from "svelte";
import { createAudioEngineHost } from "@recast/editor";
import { pushState } from "$app/navigation";
import { page } from "$app/state";
import Logo from "$lib/logo.svelte";
import { Button } from "@recast/ui/button";
import { Spinner } from "@recast/ui/spinner";
import { AlertTriangle, Download, Sparkles, Upload } from "@recast/icons";
// Type-only: a value import here would statically bind the whole editor and
// defeat the split below (rolldown reports INEFFECTIVE_DYNAMIC_IMPORT).
import type { Editor as EditorComponent, EditorStore, PanelTab } from "@recast/editor";
import type { TileProvider } from "@recast/editor/lib/timeline/filmstrip-source";
// Value import, but a leaf module with no editor deps — it doesn't drag the
// editor bundle into the landing chunk.
import { decoderBudget } from "@recast/editor/lib/playback/decoder-budget";
import { ACCEPTED_EXTENSIONS, probeSource } from "$lib/playground/probe";
import { SAMPLE_CLIP } from "$lib/playground/sample";
import { checkSupport, type SupportVerdict } from "$lib/playground/support";
import { webEditorServices } from "$lib/playground/services";
import { playgroundSession } from "$lib/playground/session.svelte";

let dragging = $state(false);
let busy = $state(false);
let error = $state<string | null>(null);
let showDesktopCta = $state(false);
let fileInput = $state<HTMLInputElement | null>(null);

// Probed on mount, not at module scope: this route is prerendered, so the
// build-time environment would answer for every visitor.
let support = $state<SupportVerdict | null>(null);
onMount(() => {
	support = checkSupport();
});

const canEdit = $derived(support?.canEdit ?? true);
const accept = ACCEPTED_EXTENSIONS.map((e) => `.${e}`).join(",");

// --- Editor island ---
// One route, two views. History state (not a second route) drives the swap, so
// Back returns to the drop surface instead of leaving the site.
let Editor = $state<typeof EditorComponent | null>(null);
let store = $state<EditorStore | null>(null);
let panels = $state<readonly PanelTab[]>([]);
let loadError = $state(false);
let tileProvider = $state<TileProvider | null>(null);
let filmstripVersion = $state(0);

const editing = $derived(Boolean(page.state.playgroundEditing) && playgroundSession.ready);

/** Clip-bar height (h-12) in CSS px; tiles decode at device pixels. */
const FILMSTRIP_TILE_HEIGHT = 48;

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
	pushState("", { playgroundEditing: true });
	await mountEditor();
	busy = false;
}

async function mountEditor() {
	loadError = false;
	try {
		const m = await import("@recast/editor");
		// The package never spawns workers; this app owns every `new Worker`.
		const { workerHost } = await import("$lib/workers");
		m.setEditorHostHooks({ workers: workerHost });
		const next = m.createEditorStore();
		const meta = playgroundSession.metadata!;
		next.metadata = { ...meta, codec: "", sizeBytes: playgroundSession.source!.file.size };
		next.loadRenderState({});
		store = next;
		panels = m.WEB_PANEL_TABS;
		Editor = m.Editor;
	} catch {
		loadError = true;
		return;
	}
	void buildTileProvider();
}

// Filmstrip thumbnails decode in their own worker off the picked File — the
// main thread never holds the bytes.
async function buildTileProvider() {
	const ref = playgroundSession.videoRef;
	if (!ref) return;
	const { createTileProvider } = await import("@recast/editor/lib/timeline/filmstrip-source");
	const dpr = window.devicePixelRatio || 1;
	const provider = await createTileProvider({
		src: ref,
		sizeBytes: playgroundSession.source?.file.size,
		durationSec: playgroundSession.metadata?.duration,
		tileHeightPx: Math.round(FILMSTRIP_TILE_HEIGHT * dpr),
		onChange: () => filmstripVersion++,
	});
	if (!editing) {
		provider?.dispose();
		return;
	}
	tileProvider = provider;
	if (provider) {
		unregisterLease = decoderBudget.registerSecondary({
			onPause: (paused) => provider.setDecodePaused(paused),
		});
	}
}

// Preview owns decode priority: the shared budget pauses the filmstrip decoder
// while the preview is playing or scrubbing, so the two never over-subscribe
// the GPU's decode sessions.
let unregisterLease: (() => void) | null = null;
let scrubBusyTimer: ReturnType<typeof setTimeout> | undefined;
let lastPreviewTime = -1;
$effect(() => {
	const s = store;
	if (!s) return;
	const playing = s.isPlaying;
	const ct = s.currentTime;
	if (playing) {
		lastPreviewTime = ct;
		decoderBudget.setPreviewBusy(true);
		return;
	}
	if (ct !== lastPreviewTime) {
		lastPreviewTime = ct;
		decoderBudget.setPreviewBusy(true);
		clearTimeout(scrubBusyTimer);
		scrubBusyTimer = setTimeout(() => decoderBudget.setPreviewBusy(false), 300);
	} else {
		decoderBudget.setPreviewBusy(false);
	}
});

// Popping back to the drop surface tears the editor down; a fresh pick rebuilds
// it. Keeping a detached store alive across the swap only risks a stale decoder.
$effect(() => {
	if (editing) return;
	untrack(() => {
		if (store || tileProvider) teardown();
	});
});

function teardown() {
	unregisterLease?.();
	unregisterLease = null;
	clearTimeout(scrubBusyTimer);
	tileProvider?.dispose();
	tileProvider = null;
	store = null;
	Editor = null;
	loadError = false;
}

onDestroy(teardown);

function onDrop(event: DragEvent) {
	event.preventDefault();
	dragging = false;
	if (!canEdit) return;
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

// Only warn once there is work to lose: an untouched clip is no loss.
function beforeUnload(event: BeforeUnloadEvent) {
	if (!editing || !playgroundSession.dirty) return;
	event.preventDefault();
}

const audioTracks = $derived(
	playgroundSession.videoRef
		? [{ src: playgroundSession.videoRef, kind: "system" as const }]
		: undefined,
);
// The editor no longer builds this: an AudioContext is host-owned so a host
// driving its own transport can't race a second engine.
const audio = createAudioEngineHost(() => audioTracks);
</script>

<svelte:window onbeforeunload={beforeUnload} />

<svelte:head>
	<title>Video editor playground — edit a clip in your browser | Recast</title>
	<meta
		name="description"
		content="Try Recast's video editor in your browser. Drop in an MP4 or WebM and add backgrounds, zoom, annotations and captions, then export — no upload, no account. Your file never leaves your device."
	/>
</svelte:head>

{#if editing}
	<!-- Chromeless (see layout.logic.ts): the editor carries its own toolbar. -->
	<div class="h-dvh w-full overflow-hidden">
		{#if loadError}
			<div class="flex h-full flex-col items-center justify-center gap-4">
				<p class="text-muted-foreground">The editor failed to load.</p>
				<div class="flex gap-2">
					<Button variant="secondary" onclick={() => void mountEditor()}>Try again</Button>
					<Button variant="ghost" onclick={() => history.back()}>Start over</Button>
				</div>
			</div>
		{:else if Editor && store}
			<Editor
				{store}
				services={webEditorServices}
				videoSrc={playgroundSession.source!.objectUrl}
				video={playgroundSession.videoRef ?? undefined}
				cameraSrc={playgroundSession.camera?.objectUrl ?? ""}
				cameraPath={playgroundSession.camera ? playgroundSession.camera.objectUrl : null}
				audioEngine={audio.current}
				{panels}
				{tileProvider}
				{filmstripVersion}
				filename={playgroundSession.source!.file.name}
			/>
		{:else}
			<div class="flex h-full flex-col items-center justify-center gap-3">
				<Spinner />
				<p class="text-muted-foreground text-sm">Loading the editor…</p>
			</div>
		{/if}
	</div>
{:else}
	<!-- The site chrome is off for the whole route, so the drop surface carries
	     its own way back to the marketing site. -->
	<div
		aria-hidden="true"
		class="bg-grid bg-grid-fade pointer-events-none fixed inset-0 -z-10 opacity-30"
	></div>

	<header class="mx-auto flex w-full max-w-3xl items-center justify-between px-6 py-6">
		<a
			href="/"
			class="group/logo flex shrink-0 items-center gap-2.5 rounded-xl transition-transform active:scale-[0.97]"
			aria-label="Recast home"
		>
			<span
				class="bg-foreground text-background shadow-craft-sm grid size-7 place-items-center rounded-lg p-1 transition-transform group-hover/logo:rotate-[-4deg]"
			>
				<Logo size="20" color="transparent" fill="currentColor" />
			</span>
			<span class="text-foreground text-[15px] font-semibold tracking-tight">Recast</span>
		</a>
		<Button variant="ghost" size="sm" href="/download">
			<Download class="size-4" />
			Get the desktop app
		</Button>
	</header>

	<main class="mx-auto flex w-full max-w-3xl flex-col gap-10 px-6 pt-10 pb-20">
		<div class="flex flex-col gap-4 text-center">
			<h1 class="text-display-md font-semibold text-balance">The Recast editor, in your browser</h1>
			<p class="text-muted-foreground mx-auto max-w-xl text-pretty">
				Drop in a clip and try the real editor — backgrounds, zoom, annotations, captions and a
				proper timeline. Nothing is uploaded; your file is decoded on your own machine.
			</p>
		</div>

		<!-- Support is stated BEFORE a file is picked: finding out after choosing a
		     clip and waiting through a probe is the worse order. -->
		{#if support?.message}
			<div
				class="flex items-start gap-3 rounded-lg border px-4 py-3 text-sm {support.level ===
				'unsupported'
					? 'border-destructive/30 bg-destructive/5 text-destructive'
					: 'border-warning/30 bg-warning/5 text-warning'}"
				role="status"
			>
				<AlertTriangle class="mt-0.5 size-4 shrink-0" />
				<p class="min-w-0 flex-1 text-pretty">{support.message}</p>
			</div>
		{/if}

		<!-- The dropzone is a button so keyboard and screen-reader users get the same
		     affordance as a drag. -->
		<button
			type="button"
			class="border-border-low hover:border-primary/60 focus-visible:border-primary flex flex-col items-center gap-3 rounded-xl border-2 border-dashed px-6 py-16 transition-colors {dragging
				? 'border-primary bg-primary/5'
				: ''}"
			disabled={busy || !canEdit}
			onclick={() => fileInput?.click()}
			ondragover={(e) => {
				e.preventDefault();
				dragging = true;
			}}
			ondragleave={() => (dragging = false)}
			ondrop={onDrop}
		>
			<Upload class="text-muted-foreground size-8" />
			<span class="font-medium">
				{busy ? "Reading your clip…" : "Drop a video, or click to choose"}
			</span>
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
			<Button variant="secondary" disabled={busy || !canEdit} onclick={loadSample}>
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
{/if}
