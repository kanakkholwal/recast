<script lang="ts">
	import { browser } from "$app/environment";
	import SeoMeta from "$lib/components/SeoMeta.svelte";
	import Container from "$lib/components/Container.svelte";
	import { evaluateTool, type CapabilityStatus } from "$lib/tools/capabilities";
	import { ConvertClientError, runConversion } from "$lib/tools/client";
	import { checkFileSize, formatBytes, inputBudget, type SizeBudget } from "$lib/tools/device";
	import type { ToolControl, ToolDef } from "$lib/tools/registry";
	import {
		buildToolJsonLd,
		buildToolOptions,
		numberControlsOf,
		outputKindFor,
		resolvePhase,
		selectControlsOf,
	} from "./ToolPage.logic";
	import { Badge } from "@recast/ui/badge";
	import { Button } from "@recast/ui/button";
	import * as Card from "@recast/ui/card";
	import { Label } from "@recast/ui/label";
	import { Segmented } from "@recast/ui/segmented";
	import { SliderControl } from "@recast/ui/slider-control";
	import { Spinner } from "@recast/ui/spinner";
	import ToolTrimRange from "./ToolTrimRange.svelte";
	import {
		Download,
		FileArchive,
		Music4,
		RotateCcw,
		ShieldCheck,
		TriangleAlert,
		Upload,
		X,
	} from "@lucide/svelte";
	import { onMount } from "svelte";

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
	let dragOver = $state(false);
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
	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
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
			const out = await runConversion(file, tool.op, buildToolOptions(tool, selectControls, numberControls, selectValues, numberValues), {
				signal: controller.signal,
				onProgress: (r) => (progress = r),
			});
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
	const segmentedOptions = (c: ToolControl) => (c.options ?? []).map((o) => ({ value: o.value, label: o.label }));

	const jsonLd = $derived(buildToolJsonLd(tool));

	// --- Preview media + direct manipulation ---------------------------------
	// The registry gives no max for the trim bounds (it can't: it depends on the
	// file). Read it off the loaded media so the sliders and the trim range have
	// a real ceiling instead of an invented one.
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
		// Default the out-point to the end of the media, so the first drag is a
		// refinement rather than a correction.
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

	// A new file means new media: forget the old duration so a stale ceiling
	// never leaks across files.
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

<div class="pt-24 pb-24 sm:pt-28">
	<Container size="narrow">
		<header class="mx-auto max-w-2xl text-center">
			<Badge variant="secondary" class="gap-1.5">
				<ShieldCheck class="size-3.5" /> Runs in your browser
			</Badge>
			<h1 class="mt-4 text-3xl font-semibold tracking-tight sm:text-4xl">{tool.title}</h1>
			<p class="mt-3 text-base text-muted-foreground">{tool.tagline}</p>
		</header>

		<!-- Hidden picker shared by the dropzone and the "change file" actions -->
		<input
			bind:this={fileInput}
			type="file"
			accept={tool.accept}
			onchange={onPick}
			class="hidden"
			disabled={!browser || blocked}
		/>

		<div class="mx-auto mt-10 max-w-2xl">
			{#if phase === "blocked"}
				<Card.Root class="border-amber-500/40 bg-amber-500/5">
					<Card.Content class="flex gap-3 py-6">
						<TriangleAlert class="mt-0.5 size-5 shrink-0 text-amber-500" />
						<div>
							<p class="font-medium">Not supported in this browser</p>
							<p class="mt-1 text-sm text-muted-foreground">{blockedReason}</p>
						</div>
					</Card.Content>
				</Card.Root>
			{:else if phase === "select"}
				<!-- Stage 1: upload -->
				<button
					type="button"
					onclick={() => fileInput?.click()}
					ondragover={(e) => {
						e.preventDefault();
						dragOver = true;
					}}
					ondragleave={() => (dragOver = false)}
					ondrop={onDrop}
					class="flex w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed px-6 py-16 text-center transition-colors {dragOver
						? 'border-primary bg-primary/5'
						: 'border-border bg-card/40 hover:border-primary/60 hover:bg-card/70'}"
				>
					<span class="flex size-14 items-center justify-center rounded-full bg-primary/10 text-primary">
						<Upload class="size-6" />
					</span>
					<span class="mt-5 text-base font-medium">Drag a file here, or click to choose</span>
					<span class="mt-1 text-sm text-muted-foreground">
						{#if budget}Up to about {budget.label} on this device.{/if}
					</span>
				</button>
				{#if sizeError}
					<p class="mt-4 flex items-start gap-2 text-sm text-destructive">
						<TriangleAlert class="mt-0.5 size-4 shrink-0" />
						<span>{sizeError}</span>
					</p>
				{/if}
				{#if funnelToApp}
					<p class="mt-3 text-sm text-muted-foreground">
						Large files are better in the
						<a href="/download" class="font-medium text-foreground underline underline-offset-4">Recast desktop app</a>, which has no size limit.
					</p>
				{/if}
			{:else if phase === "ready"}
				<!-- Stage 2: configure + preview. Deliberately ONE centred column, not
				     an app shell: this is a transaction (drop, tweak, convert, leave),
				     not a composition surface like the screenshot editor. The chrome
				     should weigh no more than the task, and the copy + FAQ around it
				     are what make the page rank and reassure a first-time visitor. -->
				<Card.Root>
					<Card.Content class="space-y-5 py-6">
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">{file?.name}</p>
								<p class="text-xs text-muted-foreground">{formatBytes(file?.size ?? 0)}</p>
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
												<p class="text-xs text-muted-foreground">{control.hint}</p>
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
					</Card.Content>
					<Card.Footer class="flex-col items-stretch gap-2 border-t pt-5">
						<Button size="lg" class="w-full gap-2" onclick={convert}>
							Convert to {tool.outputLabel}
						</Button>
						{#if errorMsg}
							<p class="flex items-start gap-2 text-sm text-destructive">
								<TriangleAlert class="mt-0.5 size-4 shrink-0" /><span>{errorMsg}</span>
							</p>
						{/if}
					</Card.Footer>
				</Card.Root>
			{:else if phase === "processing"}
				<!-- Stage 3: processing -->
				<Card.Root>
					<Card.Content class="space-y-5 py-10 text-center">
						<Spinner class="mx-auto size-8 text-primary" />
						<div>
							<p class="font-medium">Converting…</p>
							<p class="text-sm text-muted-foreground">This happens on your device. Keep the tab open.</p>
						</div>
						<div class="mx-auto max-w-sm space-y-2">
							<div class="h-2 w-full overflow-hidden rounded-full bg-muted">
								<div class="h-full rounded-full bg-primary transition-[width] duration-200" style="width: {Math.max(3, Math.round(progress * 100))}%"></div>
							</div>
							<p class="text-xs tabular-nums text-muted-foreground">{Math.round(progress * 100)}%</p>
						</div>
						<Button variant="outline" size="sm" class="gap-1.5" onclick={cancel}>
							<X class="size-4" /> Cancel
						</Button>
					</Card.Content>
				</Card.Root>
			{:else if phase === "result"}
				<!-- Stage 4: output -->
				<Card.Root class="border-primary/30">
					<Card.Content class="space-y-5 py-6">
						<div class="overflow-hidden rounded-lg border bg-black/5">
							{#if outputKind === "video"}
								<!-- svelte-ignore a11y_media_has_caption -->
								<video src={resultUrl} controls class="aspect-video w-full bg-black"></video>
							{:else if outputKind === "image"}
								<img src={resultUrl} alt="Converted result" class="mx-auto max-h-[60vh] w-full object-contain" />
							{:else if outputKind === "audio"}
								<div class="flex items-center gap-3 p-5">
									<Music4 class="size-8 shrink-0 text-muted-foreground" />
									<audio src={resultUrl} controls class="w-full"></audio>
								</div>
							{:else}
								<div class="flex items-center gap-3 p-5">
									<FileArchive class="size-8 shrink-0 text-muted-foreground" />
									<p class="text-sm text-muted-foreground">Your images are ready as a ZIP.</p>
								</div>
							{/if}
						</div>
						<div class="flex items-center justify-between gap-4">
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">{resultName}</p>
								<p class="text-xs text-muted-foreground">{formatBytes(resultSize)}</p>
							</div>
							<Button href={resultUrl} download={resultName} class="shrink-0 gap-2">
								<Download class="size-4" /> Download
							</Button>
						</div>
					</Card.Content>
					<Card.Footer class="border-t pt-5">
						<Button variant="ghost" size="sm" class="gap-1.5" onclick={startOver}>
							<RotateCcw class="size-4" /> Convert another file
						</Button>
					</Card.Footer>
				</Card.Root>
			{/if}
		</div>

		<!-- FAQ (also feeds the JSON-LD above) -->
		<div class="mx-auto mt-16 max-w-2xl">
			<h2 class="mb-5 text-lg font-semibold">Questions</h2>
			<dl class="space-y-5">
				{#each tool.faq as item (item.q)}
					<div>
						<dt class="font-medium">{item.q}</dt>
						<dd class="mt-1 text-sm text-muted-foreground">{item.a}</dd>
					</div>
				{/each}
			</dl>
		</div>
	</Container>
</div>
