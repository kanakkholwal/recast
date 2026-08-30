<script lang="ts">
import { formatBytes } from "@recast/editor/lib/format/bytes";
import {
	ArrowLeft,
	Bug,
	Download,
	FileArchive,
	Music4,
	RotateCcw,
	ShieldCheck,
	TriangleAlert,
	Upload,
	UserX,
	WifiOff,
	X,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import { Label } from "@recast/ui/label";
import { Segmented } from "@recast/ui/segmented";
import { SliderControl } from "@recast/ui/slider-control";
import { Spinner } from "@recast/ui/spinner";
import { onMount } from "svelte";
import { browser } from "$app/environment";
import { page } from "$app/state";
import Container from "$lib/components/Container.svelte";
import FaqList from "$lib/components/FaqList.svelte";
import Footer from "$lib/components/Footer.svelte";
import SectionLabel from "$lib/components/SectionLabel.svelte";
import SeoMeta from "$lib/components/SeoMeta.svelte";
import { type CapabilityStatus, evaluateTool } from "$lib/tools/capabilities";
import { ConvertClientError, runConversion } from "$lib/tools/client";
import { checkFileSize, inputBudget, type SizeBudget } from "$lib/tools/device";
import type { ToolControl, ToolDef } from "$lib/tools/registry";
import {
	buildIssueUrl,
	buildToolJsonLd,
	buildToolOptions,
	numberControlsOf,
	outputKindFor,
	resolvePhase,
	selectControlsOf,
} from "./ToolPage.logic";
import ToolTrimRange from "./ToolTrimRange.svelte";

let { tool }: { tool: ToolDef } = $props();

// svelte-ignore state_referenced_locally
const selectControls = selectControlsOf(tool);
// svelte-ignore state_referenced_locally
const numberControls = numberControlsOf(tool);

// svelte-ignore state_referenced_locally
let selectValues = $state<Record<string, string>>(
	Object.fromEntries(selectControls.map((c) => [c.key, String(c.default)])),
);
// svelte-ignore state_referenced_locally
let numberValues = $state<Record<string, number>>(
	Object.fromEntries(numberControls.map((c) => [c.key, Number(c.default)])),
);

let capability = $state<CapabilityStatus | null>(null); // null = still probing
let budget = $state<SizeBudget | null>(null);

let file = $state<File | null>(null);
let fileInput = $state<HTMLInputElement | null>(null);
// A depth counter: dragging over a child fires dragleave on the parent and flickered the highlight.
let dragDepth = $state(0);
const dragOver = $derived(dragDepth > 0);
let sizeError = $state<string | null>(null);
let busy = $state(false);
let progress = $state(0);
let errorMsg = $state<string | null>(null);
let funnelToApp = $state(false);
let resultUrl = $state<string | null>(null);
let resultName = $state("");
let resultMime = $state("");
let resultSize = $state(0);
let controller: AbortController | null = null;

onMount(async () => {
	budget = inputBudget();
	capability = await evaluateTool(tool.requirements);
});

// Object URL for previewing the chosen input, cleaned up when it changes.
let inputUrl = $state<string | null>(null);
$effect(() => {
	if (!file) {
		inputUrl = null;
		return;
	}
	const url = URL.createObjectURL(file);
	inputUrl = url;
	return () => URL.revokeObjectURL(url);
});

const blocked = $derived(capability?.supported === false);
const blockedReason = $derived(capability && !capability.supported ? capability.reason : null);
const phase = $derived(resolvePhase(blocked, busy, !!resultUrl, !!file));
const isVideoInput = $derived((file?.type ?? "").startsWith("video/"));
const outputKind = $derived(outputKindFor(resultMime));

const heroFacts = [
	{ icon: WifiOff, label: "Runs in your browser" },
	{ icon: ShieldCheck, label: "Nothing is uploaded" },
	{ icon: UserX, label: "No account" },
];

function acceptFile(f: File | null | undefined) {
	resetResult();
	sizeError = null;
	errorMsg = null;
	funnelToApp = false;
	if (!f) {
		file = null;
		return;
	}
	if (budget) {
		const check = checkFileSize(f.size, budget);
		if (!check.ok) {
			file = null;
			sizeError = check.reason ?? "This file is too large for this device.";
			funnelToApp = true;
			return;
		}
	}
	file = f;
}

function onPick(e: Event) {
	acceptFile((e.target as HTMLInputElement).files?.[0]);
}
function onDragEnter(e: DragEvent) {
	e.preventDefault();
	dragDepth++;
}
function onDragLeave() {
	dragDepth = Math.max(0, dragDepth - 1);
}
function onDrop(e: DragEvent) {
	e.preventDefault();
	dragDepth = 0;
	acceptFile(e.dataTransfer?.files?.[0]);
}

function resetResult() {
	if (resultUrl) URL.revokeObjectURL(resultUrl);
	resultUrl = null;
	resultName = "";
	resultMime = "";
	resultSize = 0;
	progress = 0;
}

function startOver() {
	resetResult();
	errorMsg = null;
	sizeError = null;
	file = null;
	if (fileInput) fileInput.value = "";
}

async function convert() {
	if (!file || blocked) return;
	busy = true;
	errorMsg = null;
	resetResult();
	controller = new AbortController();
	try {
		const out = await runConversion(
			file,
			tool.op,
			buildToolOptions(tool, selectControls, numberControls, selectValues, numberValues),
			{
				signal: controller.signal,
				onProgress: (r) => (progress = r),
			},
		);
		resultUrl = URL.createObjectURL(out.blob);
		resultName = out.filename;
		resultMime = out.mime;
		resultSize = out.blob.size;
	} catch (err) {
		if (err instanceof ConvertClientError && err.code === "cancelled") {
			// cancelled by the user — no error
		} else if (err instanceof ConvertClientError && err.code === "too-large") {
			errorMsg = err.message;
			funnelToApp = true;
		} else {
			errorMsg = err instanceof Error ? err.message : "Something went wrong converting the file.";
		}
	} finally {
		busy = false;
		controller = null;
	}
}

const cancel = () => controller?.abort();
const segmentedOptions = (c: ToolControl) =>
	(c.options ?? []).map((o) => ({ value: o.value, label: o.label }));

const jsonLd = $derived(buildToolJsonLd(tool, page.url.origin));

// The report link carries the browser string, because 'it didn't work' without it is unactionable.
const issueUrl = $derived(buildIssueUrl(tool, browser ? navigator.userAgent : ""));

// --- Preview media: the registry can't know the trim ceiling, so read it off the loaded media.
let mediaEl = $state<HTMLMediaElement | null>(null);
let duration = $state(0);
let currentTime = $state(0);

const isTrim = $derived(tool.op === "trim");
const hasTrimBounds = $derived("startSec" in numberValues && "endSec" in numberValues);
/** The drag handles own the bounds once they render, so the equivalent
 * sliders are suppressed. If metadata never loads, the sliders stay as the
 * fallback rather than leaving trim with no controls at all. */
const trimRangeShown = $derived(isTrim && hasTrimBounds && duration > 0);

function onMeta() {
	const d = mediaEl?.duration;
	if (!d || !Number.isFinite(d)) return;
	duration = d;
	// Default the out-point to the end of the media, so the first drag is a refinement, not a correction.
	if (hasTrimBounds) {
		numberValues.startSec = 0;
		numberValues.endSec = Math.round(d * 10) / 10;
	}
}

/** A slider ceiling for a control the registry left open-ended. */
function maxFor(c: ToolControl): number {
	if (c.max !== undefined) return c.max;
	if ((c.key === "startSec" || c.key === "endSec") && duration > 0) {
		return Math.ceil(duration);
	}
	return Math.max(100, Number(c.default) * 4);
}

function setTrim(next: { start: number; end: number }) {
	numberValues.startSec = Math.round(next.start * 10) / 10;
	numberValues.endSec = Math.round(next.end * 10) / 10;
}

function seek(seconds: number) {
	if (mediaEl) mediaEl.currentTime = seconds;
}

// A new file means new media, so forget the old duration or a stale ceiling leaks across files.
$effect(() => {
	void file;
	duration = 0;
	currentTime = 0;
});
</script>

<SeoMeta title={tool.title} description={tool.description} eyebrow="Free tool" />
<svelte:head>
	{@html `<script type="application/ld+json">${jsonLd}</` + `script>`}
</svelte:head>

<main class="text-foreground">
	<section class="mx-auto w-full max-w-3xl border-b border-border-low pt-32 md:pt-40">
		<Container class="pb-10">
			<!-- Tool pages are entered straight from search, so the way back to the
			     rest of the catalogue has to be on the page, not just in the nav. -->
			<a
				href="/tools"
				class="group/back inline-flex items-center gap-1.5 text-body-sm font-medium text-muted-foreground transition-colors hover:text-foreground motion-reduce:transition-none"
			>
				<ArrowLeft
					class="size-3.5 transition-transform group-hover/back:-translate-x-0.5 motion-reduce:transition-none"
				/>
				All tools
			</a>
			<div class="mt-6">
				<SectionLabel icon={ShieldCheck} label="Free tool" accent="green" />
			</div>
			<h1 class="mt-5 font-display text-balance text-heading-lg">{tool.title}</h1>
			<p class="mt-4 max-w-xl text-pretty text-body-lg text-muted-foreground">{tool.tagline}</p>
		</Container>

		<Container class="border-t border-border-low">
			<ul class="flex flex-wrap items-center divide-x divide-border-low py-4">
				{#each heroFacts as fact (fact.label)}
					{@const Icon = fact.icon}
					<li
						class="inline-flex items-center gap-2 pr-4 text-body-sm text-muted-foreground not-first:pl-4"
					>
						<Icon class="size-4 shrink-0" />
						{fact.label}
					</li>
				{/each}
			</ul>
		</Container>
	</section>

	<!-- Hidden picker shared by the dropzone and the "change file" actions -->
	<input
		bind:this={fileInput}
		type="file"
		accept={tool.accept}
		onchange={onPick}
		class="hidden"
		disabled={!browser || blocked}
	/>

	<section class="mx-auto w-full max-w-3xl border-b border-border-low">
		<Container class="py-10">
			{#if phase === "blocked"}
				<div class="flex gap-3 rounded-xl border border-border-low bg-paper p-6">
					<TriangleAlert class="mt-0.5 size-5 shrink-0 text-warning" />
					<div>
						<p class="font-display text-body font-medium text-foreground">
							Not supported in this browser
						</p>
						<p class="mt-1 text-body-sm text-muted-foreground">{blockedReason}</p>
					</div>
				</div>
			{:else if phase === "select"}
				<!-- Stage 1: upload -->
				<button
					type="button"
					onclick={() => fileInput?.click()}
					ondragenter={onDragEnter}
					ondragover={(e) => e.preventDefault()}
					ondragleave={onDragLeave}
					ondrop={onDrop}
					class="flex w-full flex-col items-center justify-center rounded-xl border border-dashed px-6 py-16 text-center transition-colors motion-reduce:transition-none {dragOver
						? 'border-primary bg-primary/8'
						: 'border-border-low bg-paper hover:border-border-strong'}"
				>
					<Upload class="size-6 text-muted-foreground" />
					<span class="mt-4 font-display text-body font-medium text-foreground">
						Drag a file here, or click to choose
					</span>
					<span class="mt-1 text-body-sm text-muted-foreground">
						{#if budget}Up to about {budget.label} on this device.{/if}
					</span>
				</button>
				{#if sizeError}
					<p class="mt-4 flex items-start gap-2 text-body-sm text-destructive">
						<TriangleAlert class="mt-0.5 size-4 shrink-0" />
						<span>{sizeError}</span>
					</p>
				{/if}
				{#if funnelToApp}
					<p class="mt-3 text-body-sm text-muted-foreground">
						Large files are better in the
						<a href="/download" class="font-medium text-foreground underline underline-offset-4">
							Recast desktop app
						</a>, which has no size limit.
					</p>
				{/if}
			{:else if phase === "ready"}
				<!-- Stage 2: configure + preview. Deliberately ONE column, not an app
				     shell: this is a transaction (drop, tweak, convert, leave), not a
				     composition surface like the screenshot editor. -->
				<div class="rounded-xl border border-border-low">
					<div class="space-y-5 p-5">
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<p class="truncate text-body-sm font-medium text-foreground">{file?.name}</p>
								<p class="text-caption tabular-nums text-muted-foreground">
									{formatBytes(file?.size ?? 0)}
								</p>
							</div>
							<Button variant="ghost" size="sm" class="shrink-0 gap-1.5" onclick={startOver}>
								<X class="size-4" /> Change
							</Button>
						</div>

						{#if inputUrl && isVideoInput}
							<!-- svelte-ignore a11y_media_has_caption -->
							<video
								bind:this={mediaEl}
								src={inputUrl}
								controls
								onloadedmetadata={onMeta}
								ontimeupdate={() => (currentTime = mediaEl?.currentTime ?? 0)}
								class="max-h-[50vh] w-full rounded-lg bg-black"
							></video>
						{:else if inputUrl}
							<audio
								bind:this={mediaEl}
								src={inputUrl}
								controls
								onloadedmetadata={onMeta}
								ontimeupdate={() => (currentTime = mediaEl?.currentTime ?? 0)}
								class="w-full"
							></audio>
						{/if}

						<!-- Trim is the one tool where typing seconds is genuinely worse
						     than dragging, so it gets real in/out handles on the media. -->
						{#if trimRangeShown}
							<ToolTrimRange
								{duration}
								{currentTime}
								start={numberValues.startSec}
								end={numberValues.endSec}
								onchange={setTrim}
								onseek={seek}
							/>
						{/if}

						{#if tool.controls?.length}
							<div class="space-y-4">
								{#each tool.controls as control (control.key)}
									{#if control.type === "select"}
										<div class="space-y-1.5">
											<Label>{control.label}</Label>
											<Segmented
												options={segmentedOptions(control)}
												value={selectValues[control.key]}
												onValueChange={(v) => (selectValues[control.key] = v)}
											/>
											{#if control.hint}
												<p class="text-caption text-muted-foreground">{control.hint}</p>
											{/if}
										</div>
									{:else if !(trimRangeShown && (control.key === "startSec" || control.key === "endSec"))}
										<!-- Drag, don't type. SliderControl still allows click-to-edit
										     for anyone who wants to key in an exact value. -->
										<SliderControl
											label={control.label}
											value={numberValues[control.key]}
											min={control.min ?? 0}
											max={maxFor(control)}
											step={control.step ?? 1}
											description={control.hint}
											onchange={(v) => (numberValues[control.key] = v)}
										/>
									{/if}
								{/each}
							</div>
						{/if}
					</div>
					<div class="flex flex-col items-stretch gap-2 border-t border-border-low p-5">
						<Button size="lg" variant="dark" class="w-full gap-2" onclick={convert}>
							Convert to {tool.outputLabel}
						</Button>
						{#if errorMsg}
							<p class="flex items-start gap-2 text-body-sm text-destructive">
								<TriangleAlert class="mt-0.5 size-4 shrink-0" /><span>{errorMsg}</span>
							</p>
						{/if}
					</div>
				</div>
			{:else if phase === "processing"}
				<!-- Stage 3: processing -->
				<div class="space-y-5 rounded-xl border border-border-low p-5 py-10 text-center">
					<Spinner class="mx-auto size-8 text-muted-foreground" />
					<div>
						<p class="font-display text-body font-medium text-foreground">Converting…</p>
						<p class="text-body-sm text-muted-foreground">
							This happens on your device. Keep the tab open.
						</p>
					</div>
					<div class="mx-auto max-w-sm space-y-2">
						<div
							class="h-2 w-full overflow-hidden rounded-full bg-paper"
							role="progressbar"
							aria-label="Conversion progress"
							aria-valuenow={Math.round(progress * 100)}
							aria-valuemin={0}
							aria-valuemax={100}
						>
							<div
								class="h-full rounded-full bg-foreground transition-[width] duration-200 motion-reduce:transition-none"
								style="width: {Math.max(3, Math.round(progress * 100))}%"
							></div>
						</div>
						<p class="text-caption tabular-nums text-muted-foreground">
							{Math.round(progress * 100)}%
						</p>
					</div>
					<Button variant="outline" size="sm" class="gap-1.5" onclick={cancel}>
						<X class="size-4" /> Cancel
					</Button>
				</div>
			{:else if phase === "result"}
				<!-- Stage 4: output -->
				<div class="rounded-xl border border-border-strong">
					<div class="space-y-5 p-5">
						<div class="overflow-hidden rounded-lg border border-border-low bg-paper">
							{#if outputKind === "video"}
								<!-- svelte-ignore a11y_media_has_caption -->
								<video src={resultUrl} controls class="aspect-video w-full bg-black"></video>
							{:else if outputKind === "image"}
								<img
									src={resultUrl}
									alt="Converted result"
									class="mx-auto max-h-[60vh] w-full object-contain"
								/>
							{:else if outputKind === "audio"}
								<div class="flex items-center gap-3 p-5">
									<Music4 class="size-6 shrink-0 text-muted-foreground" />
									<audio src={resultUrl} controls class="w-full"></audio>
								</div>
							{:else}
								<div class="flex items-center gap-3 p-5">
									<FileArchive class="size-6 shrink-0 text-muted-foreground" />
									<p class="text-body-sm text-muted-foreground">Your images are ready as a ZIP.</p>
								</div>
							{/if}
						</div>
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<p class="truncate text-body-sm font-medium text-foreground">{resultName}</p>
								<p class="text-caption tabular-nums text-muted-foreground">
									{formatBytes(resultSize)}
								</p>
							</div>
							<Button
								href={resultUrl}
								download={resultName}
								variant="dark"
								class="shrink-0 gap-2"
							>
								<Download class="size-4" /> Download
							</Button>
						</div>
					</div>
					<div class="border-t border-border-low p-5">
						<Button variant="ghost" size="sm" class="gap-1.5" onclick={startOver}>
							<RotateCcw class="size-4" /> Convert another file
						</Button>
					</div>
				</div>
			{/if}
		</Container>
	</section>

	<!-- FAQ (also feeds the JSON-LD above). Same accordion as every other page. -->
	<section class="mx-auto w-full max-w-3xl border-b border-border-low">
		<Container class="py-10">
			<h2 class="mb-5 font-display text-heading-sm">Questions</h2>
			<FaqList items={tool.faq} />
		</Container>
	</section>

	<section class="mx-auto w-full max-w-3xl">
		<Container class="py-10">
			<h2 class="font-display text-heading-sm">More free tools</h2>
			<p class="mt-2 max-w-lg text-pretty text-body-sm text-muted-foreground">
				Every conversion runs the same way: on your device, with nothing uploaded.
			</p>
			<div class="mt-6 flex flex-wrap items-center gap-3">
				<Button href="/tools" variant="dark" class="gap-2">Browse all tools</Button>
				<Button href="/download" variant="outline" class="gap-2">
					<Download class="size-4" />
					Get the desktop app
				</Button>
			</div>

			<!-- Pre-filled so a report arrives with the facts we would ask for. -->
			<p class="mt-6 text-body-sm text-muted-foreground">
				Something wrong with this tool?
				<a
					href={issueUrl}
					target="_blank"
					rel="noopener noreferrer"
					class="inline-flex items-center gap-1 font-medium text-foreground underline-offset-4 hover:underline"
				>
					<Bug class="size-3.5" />
					Report an issue
				</a>
			</p>
		</Container>
	</section>

	<Footer />
</main>
