<script lang="ts">
import type { IconComponent } from "@recast/icons";
import {
	AppWindow,
	Check,
	ChevronDown,
	Cpu,
	Mic,
	Minus,
	MonitorCog,
	MonitorOff,
	MonitorPlay,
	MousePointer2,
	RefreshCw,
	Sparkles,
	SquareDashed,
	Video,
	Volume2,
	X,
	Zap,
} from "@recast/icons";
import { Button } from "@recast/ui/button";
import * as Collapsible from "@recast/ui/collapsible";
import { cn } from "@recast/ui/utils";
import { onMount } from "svelte";
import {
	type CaptureCapabilities,
	captureCapabilities,
	diagnoseFfmpeg,
	type EncoderAvailability,
	type FfmpegDiagnostics,
	probeVideoEncoders,
} from "$lib/ipc";
import {
	buildFacts,
	captureHeadlineNote as captureHeadlineNoteOf,
	deriveOsDetail,
	deriveOsName,
	groupEncoders,
	PLATFORM_LABEL,
} from "./DeviceCapabilities.logic";

let osLabel = $state("Unknown");
let osVersion = $state("");
let osArch = $state("");
// The raw platform key, so rows can branch without string-matching the localized label; empty until loadOsInfo resolves.
let platform = $state("");

let diagnostics = $state<FfmpegDiagnostics | null>(null);
let encoders = $state<EncoderAvailability[]>([]);
let probing = $state(true);
let probeError = $state<string | null>(null);

// What this device's native APIs can actually record, probed at runtime rather than hardcoded per platform.
let captureCaps = $state<CaptureCapabilities | null>(null);
let captureProbing = $state(true);
let captureError = $state<string | null>(null);

async function loadOsInfo() {
	try {
		const os = await import("@tauri-apps/plugin-os");
		try {
			const p = os.platform();
			platform = p;
			osLabel = PLATFORM_LABEL[p] ?? p;
		} catch {
			/* leave default */
		}
		try {
			osVersion = os.version();
		} catch {
			/* optional */
		}
		try {
			osArch = os.arch();
		} catch {
			/* optional */
		}
	} catch {
		// Not running under Tauri (browser preview), leave the defaults.
	}
}

async function loadEngine() {
	probing = true;
	probeError = null;
	try {
		// The encoder matrix spawns ffmpeg per hardware candidate (~2s cold), so start both and let it fill in later.
		const [diag, enc] = await Promise.all([
			diagnoseFfmpeg().catch(() => null),
			probeVideoEncoders(),
		]);
		diagnostics = diag;
		encoders = enc;
	} catch (e) {
		probeError = String(e);
	} finally {
		probing = false;
	}
}

async function loadCapture() {
	captureProbing = true;
	captureError = null;
	try {
		captureCaps = await captureCapabilities();
	} catch (e) {
		captureError = String(e);
	} finally {
		captureProbing = false;
	}
}

onMount(() => {
	void loadOsInfo();
	void loadEngine();
	void loadCapture();
});

const osName = $derived(deriveOsName(platform, osVersion, osLabel));
const osDetail = $derived(deriveOsDetail(platform, osVersion));

// Screen is the headline verdict; audio/camera/cursor hang off the list below.
const screenCap = $derived(captureCaps?.capabilities.find((c) => c.key === "screen") ?? null);
const captureReady = $derived(screenCap?.supported ?? false);
const captureHeadlineNote = $derived(captureHeadlineNoteOf(screenCap?.note, captureReady));
let showCapture = $state(false);

// Keyed by the Rust `key`; falls back to the screen glyph for unknown keys.
const CAP_ICON: Record<string, IconComponent> = {
	screen: MonitorPlay,
	window: AppWindow,
	region: SquareDashed,
	systemAudio: Volume2,
	microphone: Mic,
	camera: Video,
	cursor: MousePointer2,
};

const facts = $derived(buildFacts(platform, osName, osDetail, osArch, diagnostics?.version));

const encoderGroups = $derived(groupEncoders(encoders));

// Which encoder the recorder actually picked, and whether it's a GPU path.
const activeEncoder = $derived(encoders.find((e) => e.active) ?? null);
const isAccelerated = $derived(activeEncoder?.hardware ?? false);
let showDetails = $state(false);
</script>

<div class="flex flex-col gap-3">
  <div
  >
    <div class="flex items-center gap-2 border-b border-border/40 px-4 py-2.5">
      <MonitorCog class="size-3.5 text-muted-foreground" />
      <span class="text-[11px] font-semibold text-foreground">Platform</span>
    </div>
    <dl class="divide-y divide-border/30">
      {#each facts as fact (fact.label)}
        <div class="flex items-center justify-between gap-3 px-4 py-2.5">
          <dt class="text-[11.5px] text-muted-foreground">{fact.label}</dt>
          <dd
            class="min-w-0 truncate font-mono text-[11px] text-foreground"
            title={fact.value}
          >
            {fact.value}
          </dd>
        </div>
      {/each}
    </dl>
  </div>

  <!-- Probed at runtime (DXGI / AVFoundation / PipeWire / X11), not hardcoded. -->
  <div
  >
    <div class="flex items-center gap-2 border-b border-border/40 px-4 py-2.5">
      <MonitorPlay class="size-3.5 text-muted-foreground" />
      <span class="text-[11px] font-semibold text-foreground">
        Capture support
      </span>
    </div>

    {#if captureProbing && !captureCaps}
      <div class="flex items-center gap-3 px-4 py-3.5">
        <div class="size-9 shrink-0 animate-pulse rounded-full bg-foreground/5"></div>
        <div class="flex-1 space-y-1.5">
          <div class="h-3 w-36 animate-pulse rounded bg-foreground/5"></div>
          <div class="h-2.5 w-full max-w-60 animate-pulse rounded bg-foreground/5"></div>
        </div>
      </div>
    {:else if captureError}
      <div class="px-4 py-3 text-[11px] text-destructive">
        Couldn't check capture support: {captureError}
      </div>
    {:else if captureCaps}
      <div class="flex items-start gap-3 px-4 py-3.5">
        <div
          class={cn(
            "flex size-9 shrink-0 items-center justify-center rounded-full ring-1 ring-inset",
            captureReady
              ? "bg-muted/60 text-foreground ring-border/40"
              : "bg-warning/12 text-warning ring-1 ring-warning/25",
          )}
        >
          {#if captureReady}
            <MonitorPlay class="size-4" />
          {:else}
            <MonitorOff class="size-4" />
          {/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span class="text-[13px] font-semibold text-foreground">
              {captureReady
                ? "Screen recording is ready"
                : "Screen recording isn't available here"}
            </span>
            <span
              class={cn(
                "inline-flex items-center rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide ring-1 ring-inset",
                captureReady
                  ? "bg-muted/60 text-foreground ring-border/40"
                  : "bg-warning/12 text-warning ring-warning/25",
              )}
            >
              {captureCaps.screenBackend}
            </span>
          </div>
          <p class="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
            {captureHeadlineNote}
          </p>
        </div>
      </div>

      <!-- Per-feature matrix, collapsed by default; one row per capture input. -->
      <Collapsible.Root bind:open={showCapture}>
        <Collapsible.Trigger
          class="flex w-full items-center justify-between gap-2 border-t border-border/30 px-4 py-2 text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground"
        >
          <span>Feature support</span>
          <ChevronDown
            class={cn("size-3.5 transition-transform", showCapture && "rotate-180")}
          />
        </Collapsible.Trigger>
        <Collapsible.Content>
          <ul class="divide-y divide-border/30">
          {#each captureCaps.capabilities as feat (feat.key)}
            {@const Icon = CAP_ICON[feat.key] ?? MonitorPlay}
            <li class="flex items-start justify-between gap-3 px-4 py-2.5">
              <div class="flex min-w-0 items-start gap-2.5">
                <div
                  class={cn(
                    "mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-lg ring-1 ring-inset",
                    feat.supported
                      ? "bg-muted/60 text-foreground ring-border/40"
                      : "bg-foreground/5 text-muted-foreground/60 ring-border/40",
                  )}
                >
                  <Icon class="size-3.5" />
                </div>
                <div class="min-w-0">
                  <span class="block truncate text-[12px] font-semibold text-foreground">
                    {feat.label}
                  </span>
                  <div class="truncate font-mono text-[10px] text-muted-foreground">
                    {feat.backend}
                  </div>
                  {#if feat.note}
                    <p class="mt-0.5 text-[10.5px] leading-relaxed text-muted-foreground/80">
                      {feat.note}
                    </p>
                  {/if}
                </div>
              </div>
              <span
                class={cn(
                  "mt-0.5 inline-flex shrink-0 items-center gap-1 text-[10.5px] font-medium",
                  feat.supported ? "text-success" : "text-muted-foreground/70",
                )}
              >
                {#if feat.supported}
                  <Check class="size-3.5" />
                  Supported
                {:else}
                  <X class="size-3.5" />
                  Unavailable
                {/if}
              </span>
            </li>
          {/each}
          </ul>
        </Collapsible.Content>
      </Collapsible.Root>
    {/if}
  </div>

  <div
  >
    <div
      class="flex items-center justify-between gap-2 border-b border-border/40 px-4 py-2.5"
    >
      <div class="flex items-center gap-2">
        <Zap class="size-3.5 text-muted-foreground" />
        <span class="text-[11px] font-semibold text-foreground">
          Hardware acceleration
        </span>
      </div>
      <Button
        variant="ghost"
        size="xs"
        class="h-6 gap-1.5 text-[11px]"
        disabled={probing}
        onclick={loadEngine}
      >
        <RefreshCw class={cn("size-3", probing && "animate-spin")} />
        {probing ? "Checking…" : "Re-check"}
      </Button>
    </div>

    {#if probeError}
      <div class="px-4 py-3 text-[11px] text-destructive">
        Couldn't check hardware acceleration: {probeError}
      </div>
    {:else if probing && encoders.length === 0}
      <div class="flex items-center gap-3 px-4 py-3.5">
        <div class="size-9 shrink-0 animate-pulse rounded-full bg-foreground/5"></div>
        <div class="flex-1 space-y-1.5">
          <div class="h-3 w-32 animate-pulse rounded bg-foreground/5"></div>
          <div class="h-2.5 w-full max-w-60 animate-pulse rounded bg-foreground/5"></div>
        </div>
      </div>
    {:else}
      <div class="flex items-start gap-3 px-4 py-3.5">
        <div
          class={cn(
            "flex size-9 shrink-0 items-center justify-center rounded-full ring-1 ring-inset",
            isAccelerated
              ? "bg-muted/60 text-foreground ring-border/40"
              : "bg-foreground/5 text-muted-foreground ring-border/50",
          )}
        >
          {#if isAccelerated}
            <Zap class="size-5" />
          {:else}
            <Cpu class="size-5" />
          {/if}
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span class="text-[13px] font-semibold text-foreground">
              {isAccelerated ? "Hardware accelerated" : "Running on your CPU"}
            </span>
            <span
              class={cn(
                "inline-flex items-center rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide ring-1 ring-inset",
                isAccelerated
                  ? "bg-muted/60 text-foreground ring-border/40"
                  : "bg-foreground/5 text-muted-foreground/80 ring-border/50",
              )}
            >
              {isAccelerated ? `GPU · ${activeEncoder?.vendor ?? ""}` : "CPU only"}
            </span>
          </div>
          <p class="mt-0.5 text-[11px] leading-relaxed text-muted-foreground">
            {#if isAccelerated}
              Recordings are encoded by your {activeEncoder?.vendor} graphics
              card, so capture stays smooth and your processor stays free for
              everything else.
            {:else}
              No graphics-card encoder was available, so recordings are encoded
              by your processor (CPU). It still works well, but expect higher
              CPU use while recording.
            {/if}
          </p>
        </div>
      </div>

      <!-- Per-codec matrix, collapsed by default; power-user / bug-report detail. -->
      <Collapsible.Root bind:open={showDetails}>
        <Collapsible.Trigger
          class="flex w-full items-center justify-between gap-2 border-t border-border/30 px-4 py-2 text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground"
        >
          <span>Technical details</span>
          <ChevronDown
            class={cn("size-3.5 transition-transform", showDetails && "rotate-180")}
          />
        </Collapsible.Trigger>
        <Collapsible.Content>
          {#each encoderGroups as group (group.family)}
          <div
            class="flex items-center gap-2 border-b border-border/30 bg-muted/20 px-4 py-1.5"
          >
            <span
              class="text-[10px] font-bold uppercase tracking-[0.12em] text-muted-foreground/70"
            >
              {group.family}
            </span>
          </div>
          <ul class="divide-y divide-border/30">
            {#each group.items as enc (enc.name)}
              <li class="flex items-center justify-between gap-3 px-4 py-2.5">
                <div class="flex min-w-0 items-center gap-2.5">
                  <div
                    class={cn(
                      "flex size-7 shrink-0 items-center justify-center rounded-lg ring-1 ring-inset",
                      enc.available
                        ? "bg-muted/60 text-foreground ring-border/40"
                        : "bg-foreground/5 text-muted-foreground/60 ring-border/40",
                    )}
                  >
                    {#if enc.hardware}
                      <Zap class="size-3.5" />
                    {:else}
                      <Cpu class="size-3.5" />
                    {/if}
                  </div>
                  <div class="min-w-0">
                    <div class="flex items-center gap-1.5">
                      <span class="truncate text-[12px] font-semibold text-foreground">
                        {enc.label}
                      </span>
                      {#if enc.active}
                        <span
                          class="inline-flex items-center gap-1 rounded-full bg-primary/15 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-primary"
                        >
                          <Sparkles class="size-2.5" />
                          In use
                        </span>
                      {/if}
                    </div>
                    <div class="truncate font-mono text-[10px] text-muted-foreground">
                      {enc.name} · {enc.vendor}
                    </div>
                  </div>
                </div>
                <span
                  class={cn(
                    "inline-flex shrink-0 items-center gap-1 text-[10.5px] font-medium",
                    enc.available ? "text-success" : "text-muted-foreground/70",
                  )}
                >
                  {#if enc.available}
                    <Check class="size-3.5" />
                    Available
                  {:else}
                    <X class="size-3.5" />
                    Unsupported
                  {/if}
                </span>
              </li>
            {/each}
          </ul>
        {/each}
        <p class="border-t border-border/30 px-4 py-2.5 text-[10.5px] leading-relaxed text-muted-foreground/80">
          <Minus class="mr-0.5 inline size-3 -translate-y-px" />
          Recast records with the highest-priority available H.264 encoder.
          Hardware encoders (GPU) keep capture smooth on weaker CPUs; x264 is the
          always-on software fallback. HEVC rows are informational: which HEVC
          encoders this device exposes.
        </p>
        </Collapsible.Content>
      </Collapsible.Root>
    {/if}
  </div>
</div>
